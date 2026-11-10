use super::*;
use crate::llm::{ChatContent, ChatContentPart, ToolDefinition};
use crate::state::{estimate_chat_messages_tokens, CompactionBudgetPolicy};

impl SubagentRunner {
    /// 【子任务】【上下文维护】共用上下文策略，压缩成功且体积减小时才替换历史。
    /// 参数：对话、统计、加载状态及强制压缩标记；返回是否实际替换了历史。
    pub(super) async fn compact_context(
        &self,
        messages: &mut Vec<ChatMessage>,
        stats: &mut SubagentStats,
        visibility: &mut ToolVisibility,
        force: bool,
    ) -> Result<bool> {
        let Some((config, paths)) = &self.progressive_context else {
            return Ok(false);
        };
        let limit = config.active_context_window_tokens()?;
        if limit == 0 {
            return Ok(false);
        }
        let policy = CompactionBudgetPolicy::from_context(
            config.context.clamped_compaction_ratio(),
            config.context.compaction_reserve_tokens,
        )
        .with_mode(config.context.compaction_mode);
        let definitions = visibility.definitions(&self.tools);
        let tool_tokens =
            crate::token_estimate::estimate_tokens(&serde_json::to_string(&definitions)?);
        let before = estimate_chat_messages_tokens(messages);
        if !force && before.saturating_add(tool_tokens) < policy.trigger_chars(limit) {
            return Ok(false);
        }
        let Some(split) = compaction_boundary(messages, limit.saturating_sub(tool_tokens) / 4)
        else {
            return Ok(false);
        };
        self.progress
            .phase("【子任务】【上下文维护】正在压缩历史并保留当前任务与最近工具结果");
        let runtime = config.compaction_runtime_config()?;
        let client = OpenAiCompatibleClient::from_config(&runtime, paths)?;
        let summary_limit = crate::state::summary_char_limit(limit);
        // 1. 【子任务】【容量控制】按摘要模型窗口分块，供应商溢出时也能处理过大的历史
        let chunk_limit = runtime.active_context_window_tokens()?.min(limit) / 3;
        anyhow::ensure!(
            chunk_limit >= 256,
            "subagent compaction model context is too small"
        );
        let history = messages[2..split]
            .iter()
            .map(summary_text)
            .collect::<Vec<_>>()
            .join("\n");
        let mut summary = String::new();
        for chunk in history_chunks(&history, chunk_limit) {
            let prompt = crate::prompts::template::render_prompt_pair(
                &config.prompt.templates.compaction,
                &[
                    ("previous_summary", summary.as_str()),
                    ("history", chunk.as_str()),
                ],
            )?;
            let request = vec![ChatMessage::system(prompt.system), ChatMessage::plain("user", format!(
                "{}\nKeep all task constraints, user guidance, completed operations, unresolved work, and exact file references. Limit the complete summary to {summary_limit} characters. Do not execute tools.", prompt.user))];
            let result = client
                .chat_stream_events_retry(request.clone(), Vec::new(), &runtime.retry, |_| Ok(()))
                .await?;
            stats.record_response(&request, &result);
            if let Some(accounting) = &self.goal_accounting {
                accounting.record(result.usage.as_ref())?;
            }
            self.progress.stats(stats.public());
            anyhow::ensure!(
                result.tool_calls.is_empty(),
                "subagent compaction attempted tool execution"
            );
            crate::state::validate_summary(&result.content, summary_limit)?;
            summary = result.content;
        }
        let mut compacted = messages[..2].to_vec();
        compacted.push(ChatMessage::plain(
            "user",
            format!("<subagent-context-summary>\n{summary}\n</subagent-context-summary>"),
        ));
        compacted.extend_from_slice(&messages[split..]);
        if estimate_chat_messages_tokens(&compacted) >= before {
            return Ok(false);
        }
        // 2. 【子任务】【压缩留档】写入原文后再替换，留档失败时保留完整对话
        if let Some(id) = &self.checkpoint_id {
            let archive = crate::tools::subagent_state::archive_context(id, &messages[2..split])?;
            compacted[2] = ChatMessage::plain("user", format!("<subagent-context-summary>\n{summary}\nFull prior context: {}\n</subagent-context-summary>", archive.display()));
        }
        if estimate_chat_messages_tokens(&compacted).saturating_add(tool_tokens) >= limit {
            return Ok(false);
        }
        *messages = compacted;
        // 3. 【子任务】【加载恢复】被摘要替换的工具与技能正文允许重新加载
        visibility.restore_loaded_tools(&self.tools, &[]);
        visibility.clear_loaded_skills();
        Ok(true)
    }

    /// 【子任务】【模型请求】预算维护后请求模型，上下文溢出仅压缩重试一次。
    /// 参数：运行中对话、统计、可见性、工具声明、轮次和步数；返回完整模型响应。
    pub(super) async fn request_with_context(
        &self,
        messages: &mut Vec<ChatMessage>,
        stats: &mut SubagentStats,
        visibility: &mut ToolVisibility,
        definitions: Vec<ToolDefinition>,
        round: usize,
        steps: usize,
    ) -> Result<ChatResult> {
        self.compact_context(messages, stats, visibility, false)
            .await?;
        self.save_checkpoint(messages, stats, visibility, steps, false, &[])?;
        let result = self
            .request_model_round(messages.clone(), definitions.clone(), round)
            .await;
        match result {
            Err(error) if crate::llm::is_context_overflow_error(&error) => {
                if !self
                    .compact_context(messages, stats, visibility, true)
                    .await?
                {
                    return Err(error);
                }
                self.save_checkpoint(messages, stats, visibility, steps, false, &[])?;
                self.request_model_round(messages.clone(), definitions, round)
                    .await
            }
            other => other,
        }
    }
}

/// 【子任务】【压缩边界】保留首条任务与最近完整工具轮次，禁止从工具结果中间切割。
/// 参数：完整消息及尾部 token 预算；返回可压缩前缀末端，无法压缩时为空。
fn compaction_boundary(messages: &[ChatMessage], tail_budget: usize) -> Option<usize> {
    if messages.len() <= 2 {
        return None;
    }
    let candidates = messages
        .iter()
        .enumerate()
        .skip(3)
        .filter(|(_, message)| message.role == "assistant")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut split = *candidates.last().unwrap_or(&messages.len());
    if estimate_chat_messages_tokens(&messages[split..]) > tail_budget && tail_budget > 0 {
        split = messages.len();
    } else {
        for index in candidates.into_iter().rev().skip(1) {
            if estimate_chat_messages_tokens(&messages[index..]) > tail_budget {
                break;
            }
            split = index;
        }
    }
    // 1. 【子任务】【指导保留】最近一条真实用户指导必须保留原文，图片附件及旧摘要不算新指导
    if let Some((index, _)) = messages.iter().enumerate().skip(2).rfind(|(_, message)| {
        message.role == "user" && !crate::tools::model_attachment::is_attachment_message(message)
            && !matches!(&message.content, Some(ChatContent::Text(text)) if text.starts_with("<subagent-context-summary>"))
    }) { split = split.min(index); }
    (split > 2).then_some(split)
}

/// 【子任务】【摘要投影】只传文本与工具调用，不把图片编码作为文本输入。
/// 参数：一条消息；返回带角色及工具关联的摘要输入。
fn summary_text(message: &ChatMessage) -> String {
    let text = match &message.content {
        Some(ChatContent::Text(text)) => text.clone(),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .map(|part| match part {
                ChatContentPart::Text { text } => text.clone(),
                ChatContentPart::ImageUrl { .. } => "[image attachment]".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    };
    format!(
        "<{} tool_call_id={:?}>\n{}\n{}\n</{}>",
        message.role,
        message.tool_call_id,
        text,
        message
            .tool_calls
            .as_ref()
            .map(|calls| serde_json::to_string(calls).unwrap_or_default())
            .unwrap_or_default(),
        message.role
    )
}

/// 【子任务】【摘要分块】按实际分词结果划分完整历史，避免截断和遗漏。
/// 参数：历史正文及每块 token 上限；返回有序分块。
fn history_chunks(history: &str, tokens: usize) -> Vec<String> {
    let chars = history.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut count = (tokens.saturating_mul(2)).max(1).min(chars.len() - start);
        let chunk = loop {
            let text = chars[start..start + count].iter().collect::<String>();
            if count == 1 || crate::token_estimate::estimate_tokens(&text) <= tokens {
                break text;
            }
            count = (count / 2).max(1);
        };
        chunks.push(chunk);
        start += count;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【子任务】【边界回归】近期工具调用和结果必须一起保留，中文分块不能丢字符。
    #[test]
    fn keeps_tool_pairs_and_all_chunk_text() {
        let messages = vec![
            ChatMessage::system("system"),
            ChatMessage::plain("user", "task"),
            ChatMessage::assistant("old", None),
            ChatMessage::tool("old", "large history"),
            ChatMessage::assistant("recent", None),
            ChatMessage::tool("recent", "result"),
        ];
        assert_eq!(compaction_boundary(&messages, 0), Some(4));
        let history = "中文 guidance ".repeat(900);
        assert_eq!(history_chunks(&history, 100).concat(), history);
    }
}
