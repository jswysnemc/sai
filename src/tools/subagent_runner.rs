use super::{readable_tool_name, tool_output_for_context, ToolProgress, ToolRegistry};
use crate::i18n::is_zh;
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamKind, OpenAiCompatibleClient, Usage,
};
use anyhow::Result;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ProgressMode {
    Hidden,
    Summary,
    Full,
}

#[derive(Clone)]
pub(crate) struct SubagentProgress {
    progress: ToolProgress,
    mode: ProgressMode,
    enabled: bool,
}

impl SubagentProgress {
    /// 创建子代理进度回调封装。
    ///
    /// 参数:
    /// - `progress`: 宿主工具进度发送器
    /// - `mode`: 展示模式
    /// - `enabled`: 是否展示进度
    ///
    /// 返回:
    /// - 子代理进度对象
    pub(crate) fn new(progress: ToolProgress, mode: ProgressMode, enabled: bool) -> Self {
        Self {
            progress,
            mode,
            enabled,
        }
    }

    /// 上报阶段进度信息。
    ///
    /// 参数:
    /// - `message`: 阶段文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn phase(&self, message: impl Into<String>) {
        if self.enabled && self.mode != ProgressMode::Hidden {
            self.progress.report(message.into());
        }
    }

    /// 上报子代理推理文本。
    ///
    /// 参数:
    /// - `text`: 推理文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn reasoning(&self, text: &str) {
        if self.enabled && self.mode != ProgressMode::Hidden {
            self.progress
                .report(format!("__subagent_reasoning__{}", text));
        }
    }

    /// 上报子智能体正文流分片。
    ///
    /// 参数:
    /// - `text`: 模型实时返回的正文分片
    ///
    /// 返回:
    /// - 无
    pub(crate) fn content(&self, text: &str) {
        if self.enabled && self.mode == ProgressMode::Full && !text.is_empty() {
            self.progress.report(format!("__subagent_text__{}", text));
        }
    }

    /// 上报子工具开始运行。
    ///
    /// 参数:
    /// - `step`: 当前工具调用序号
    /// - `name`: 子工具名称
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_start(&self, step: usize, name: &str) {
        if !self.enabled || self.mode == ProgressMode::Hidden {
            return;
        }
        if self.mode == ProgressMode::Summary {
            self.progress.report(if is_zh() {
                format!("工具 #{step}：{} 运行中", readable_tool_name(name))
            } else {
                format!("tool #{step}: {name} running")
            });
        }
    }

    /// 上报子工具调用参数。
    ///
    /// 参数:
    /// - `name`: 子工具名称
    /// - `args`: 子工具参数 JSON
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_call_detail(&self, name: &str, args: &str) {
        if self.enabled && self.mode == ProgressMode::Full {
            self.progress.report(format!(
                "__subtool_call__{}",
                json!({ "name": name, "args": args })
            ));
        }
    }

    /// 上报子工具完成状态。
    ///
    /// 参数:
    /// - `step`: 当前工具调用序号
    /// - `name`: 子工具名称
    /// - `ok`: 是否成功
    /// - `output`: 子工具输出
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_end(&self, step: usize, name: &str, ok: bool, output: &str) {
        if !self.enabled || self.mode == ProgressMode::Hidden {
            return;
        }
        if self.mode == ProgressMode::Summary {
            self.progress.report(if is_zh() {
                format!("工具 #{step}：{} ok", readable_tool_name(name))
            } else {
                format!("tool #{step}: {name} ok")
            });
        }
        if self.mode == ProgressMode::Full {
            self.progress.report(format!(
                "__subtool_result__{}",
                json!({ "name": name, "ok": ok, "output": output })
            ));
        }
    }
}

#[derive(Default)]
pub(crate) struct SubagentStats {
    pub(crate) tool_calls: usize,
    pub(crate) tool_ok: usize,
    pub(crate) tool_errors: usize,
    pub(crate) prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) total_tokens: u64,
    pub(crate) token_estimate: u64,
    pub(crate) token_estimate_method: TokenEstimateMethod,
    pub(crate) budget_reached: bool,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) enum TokenEstimateMethod {
    #[default]
    None,
    ProviderUsage,
    ProviderUsagePlusEstimate,
    RoughCharEstimate,
}

impl SubagentStats {
    /// 记录模型用量，缺失 provider usage 时使用字符数粗略估计。
    ///
    /// 参数:
    /// - `usage`: provider 返回的用量
    /// - `texts`: 用于粗略估计的文本片段
    ///
    /// 返回:
    /// - 无
    pub(crate) fn add_usage_or_estimate(&mut self, usage: Option<&Usage>, texts: &[&str]) {
        if let Some(usage) = usage {
            if usage.total_tokens > 0 {
                self.prompt_tokens += usage.prompt_tokens;
                self.completion_tokens += usage.completion_tokens;
                self.total_tokens += usage.total_tokens;
                self.token_estimate += usage.total_tokens;
                self.token_estimate_method = match self.token_estimate_method {
                    TokenEstimateMethod::None | TokenEstimateMethod::ProviderUsage => {
                        TokenEstimateMethod::ProviderUsage
                    }
                    _ => TokenEstimateMethod::ProviderUsagePlusEstimate,
                };
                return;
            }
        }
        let estimate = estimate_tokens(texts);
        self.token_estimate += estimate;
        self.token_estimate_method = match self.token_estimate_method {
            TokenEstimateMethod::None | TokenEstimateMethod::RoughCharEstimate => {
                TokenEstimateMethod::RoughCharEstimate
            }
            _ => TokenEstimateMethod::ProviderUsagePlusEstimate,
        };
    }

    /// 生成可返回给主 agent 的统计信息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - JSON 统计字段
    pub(crate) fn public(&self) -> Value {
        json!({
            "tool_calls": self.tool_calls,
            "tool_ok": self.tool_ok,
            "tool_errors": self.tool_errors,
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "total_tokens": self.total_tokens,
            "token_estimate": self.token_estimate,
            "token_estimate_method": token_estimate_method_label(self.token_estimate_method),
            "token_estimate_is_actual": self.token_estimate_method == TokenEstimateMethod::ProviderUsage,
        })
    }
}

/// 返回 token 估算方式标签。
///
/// 参数:
/// - `method`: token 估算方式
///
/// 返回:
/// - 估算方式标签
pub(crate) fn token_estimate_method_label(method: TokenEstimateMethod) -> &'static str {
    match method {
        TokenEstimateMethod::ProviderUsage => "provider_usage",
        TokenEstimateMethod::ProviderUsagePlusEstimate => "provider_usage_plus_estimate",
        TokenEstimateMethod::RoughCharEstimate | TokenEstimateMethod::None => "rough_char_estimate",
    }
}

/// 依据 CJK/拉丁密度粗略估计 token 数。
///
/// 参数:
/// - `texts`: 待估计文本
///
/// 返回:
/// - 粗略 token 数
pub(crate) fn estimate_tokens(texts: &[&str]) -> u64 {
    crate::token_estimate::estimate_texts_tokens(texts)
}

/// 格式化 token 数量。
///
/// 参数:
/// - `tokens`: token 数量
/// - `estimated`: 是否为估算值
///
/// 返回:
/// - 易读 token 数量文本
pub(crate) fn format_token_count(tokens: u64, estimated: bool) -> String {
    let prefix = if estimated { "~" } else { "" };
    if tokens >= 1_000_000 {
        format!("{prefix}{:.2}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{prefix}{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{prefix}{tokens}")
    }
}

/// 生成工具预算耗尽后的收尾提示。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 收尾提示文本
pub(crate) fn finalization_prompt() -> &'static str {
    "<tool_budget_reached>工具预算已用尽。不要再请求工具。请只基于上面的任务描述和已执行工具结果输出最终结果；缺少信息的地方明确说明。</tool_budget_reached>"
}

pub(crate) struct SubagentRunner {
    client: OpenAiCompatibleClient,
    system_prompt: String,
    tools: ToolRegistry,
    excluded_tools: Vec<String>,
    max_steps: usize,
    timeout_seconds: u64,
    progress: SubagentProgress,
}

impl SubagentRunner {
    /// 创建子代理执行器。
    ///
    /// 参数:
    /// - `client`: LLM 客户端
    /// - `system_prompt`: 子代理系统提示
    /// - `tools`: 子代理可用工具注册表
    /// - `progress`: 子代理进度回调
    ///
    /// 返回:
    /// - 子代理执行器
    pub(crate) fn new(
        client: OpenAiCompatibleClient,
        system_prompt: impl Into<String>,
        tools: ToolRegistry,
        progress: SubagentProgress,
    ) -> Self {
        Self {
            client,
            system_prompt: system_prompt.into(),
            tools,
            excluded_tools: Vec::new(),
            max_steps: 0,
            timeout_seconds: 60,
            progress,
        }
    }

    /// 设置最大工具调用次数。
    ///
    /// 参数:
    /// - `n`: 最大工具调用次数，0 表示不限制
    ///
    /// 返回:
    /// - 更新后的执行器
    pub(crate) fn max_steps(mut self, n: usize) -> Self {
        self.max_steps = n;
        self
    }

    /// 设置单次工具调用超时。
    ///
    /// 参数:
    /// - `seconds`: 超时秒数
    ///
    /// 返回:
    /// - 更新后的执行器
    pub(crate) fn timeout_seconds(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// 设置需要排除的工具名称。
    ///
    /// 参数:
    /// - `names`: 排除工具名称
    ///
    /// 返回:
    /// - 更新后的执行器
    pub(crate) fn excluded_tools(mut self, names: &[&str]) -> Self {
        self.excluded_tools = names.iter().map(|name| name.to_string()).collect();
        self
    }

    /// 运行子代理任务。
    ///
    /// 参数:
    /// - `prompt`: 子代理任务提示
    ///
    /// 返回:
    /// - 子代理最终结果和统计信息
    pub(crate) async fn run(&self, prompt: &str) -> Result<(ChatResult, SubagentStats)> {
        let mut stats = SubagentStats::default();
        let messages = vec![
            ChatMessage::system(self.system_prompt.clone()),
            ChatMessage::plain("user", prompt.to_string()),
        ];
        let result = self.chat_with_tools(messages, &mut stats).await?;
        stats.add_usage_or_estimate(
            result.usage.as_ref(),
            &[&self.system_prompt, prompt, &result.content],
        );
        self.report_stats(&stats);
        Ok((result, stats))
    }

    /// 上报子代理最终统计。
    ///
    /// 参数:
    /// - `stats`: 子代理统计信息
    ///
    /// 返回:
    /// - 无
    fn report_stats(&self, stats: &SubagentStats) {
        let text = if is_zh() {
            format!(
                "工具调用 {} 次　消耗 Token {}",
                stats.tool_calls,
                format_token_count(stats.token_estimate, false)
            )
        } else {
            format!(
                "tool calls: {}　token cost: {}",
                stats.tool_calls,
                format_token_count(stats.token_estimate, false)
            )
        };
        self.progress.phase(text);
    }

    /// 运行子代理的工具调用循环。
    ///
    /// 参数:
    /// - `messages`: 初始消息
    /// - `stats`: 可变统计信息
    ///
    /// 返回:
    /// - 子代理最终聊天结果
    async fn chat_with_tools(
        &self,
        mut messages: Vec<ChatMessage>,
        stats: &mut SubagentStats,
    ) -> Result<ChatResult> {
        let excluded = self
            .excluded_tools
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let definitions = self.tools.definitions_except(&excluded);
        let mut steps = 0usize;
        loop {
            if self.max_steps > 0 && steps >= self.max_steps {
                stats.budget_reached = true;
                messages.push(ChatMessage::plain("user", finalization_prompt()));
                let result = self
                    .client
                    .chat_stream(messages, Vec::new(), |chunk: ChatStreamChunk| {
                        match chunk.kind {
                            ChatStreamKind::Reasoning => self.progress.reasoning(&chunk.text),
                            ChatStreamKind::Content => self.progress.content(&chunk.text),
                        }
                        Ok(())
                    })
                    .await?;
                stats.add_usage_or_estimate(result.usage.as_ref(), &[&result.content]);
                return Ok(result);
            }
            let result = self
                .client
                .chat_stream(
                    messages.clone(),
                    definitions.clone(),
                    |chunk: ChatStreamChunk| {
                        match chunk.kind {
                            ChatStreamKind::Reasoning => self.progress.reasoning(&chunk.text),
                            ChatStreamKind::Content => self.progress.content(&chunk.text),
                        }
                        Ok(())
                    },
                )
                .await?;
            stats.add_usage_or_estimate(result.usage.as_ref(), &[]);
            if result.tool_calls.is_empty() {
                return Ok(result);
            }
            messages.push(ChatMessage::assistant(
                result.content.clone(),
                Some(result.tool_calls.clone()),
            ));
            for call in result.tool_calls {
                if self.max_steps > 0 && steps >= self.max_steps {
                    messages.push(ChatMessage::tool(
                        call.id,
                        "tool budget reached for this subagent session",
                    ));
                    continue;
                }
                steps += 1;
                stats.tool_calls += 1;
                self.progress.tool_start(steps, &call.function.name);
                self.progress
                    .tool_call_detail(&call.function.name, &call.function.arguments);
                let (output, ok) = match tokio::time::timeout(
                    Duration::from_secs(self.timeout_seconds.max(5)),
                    self.tools
                        .call(&call.function.name, &call.function.arguments),
                )
                .await
                {
                    Ok(Ok(output)) => (output, true),
                    Ok(Err(err)) => (format!("tool error: {err}"), false),
                    Err(_) => (
                        format!(
                            "tool error: {} timed out after {}s",
                            call.function.name, self.timeout_seconds
                        ),
                        false,
                    ),
                };
                if ok {
                    stats.tool_ok += 1;
                } else {
                    stats.tool_errors += 1;
                }
                self.progress
                    .tool_end(steps, &call.function.name, ok, &output);
                messages.push(ChatMessage::tool(
                    call.id,
                    tool_output_for_context(&call.function.name, &output),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// 验证子代理用量回退与全局 BPE 估算保持一致。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn estimates_tokens_with_shared_bpe() {
        assert_eq!(estimate_tokens(&["abcd"]), 1);
        assert_eq!(estimate_tokens(&["abcdefgh"]), 1);
        assert_eq!(estimate_tokens(&["你好世界"]), 2);
        assert_eq!(estimate_tokens(&["hello ", "world"]), 2);
    }

    #[test]
    fn formats_token_counts_without_unicode_prefix() {
        assert_eq!(format_token_count(999, false), "999");
        assert_eq!(format_token_count(1_500, true), "~1.5K");
    }

    #[test]
    fn full_progress_forwards_content_chunks_immediately() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let progress = SubagentProgress::new(ToolProgress::new(sender), ProgressMode::Full, true);

        progress.content("first");
        progress.content(" second");

        assert_eq!(receiver.try_recv().unwrap(), "__subagent_text__first");
        assert_eq!(receiver.try_recv().unwrap(), "__subagent_text__ second");
    }
}
