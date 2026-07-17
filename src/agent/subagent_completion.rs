use super::message_context::system_messages_first;
use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, ChatResult, ChatStreamEvent, Usage};
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::project_provider_turn_from_messages;
use crate::tools::subagent_state::{list_subagents_for_owner, pending_finished_notices};
use crate::tools::SubagentReminder;
use anyhow::Result;
use std::time::Duration;

const COMPLETION_POLL_INTERVAL: Duration = Duration::from_millis(250);

impl Agent {
    /// 在主回复生成后等待并主动提交尚未消费的子智能体结果。
    ///
    /// 参数:
    /// - `turn_id`: 当前对话轮次标识
    /// - `messages`: 当前轮模型上下文
    /// - `initial_result`: 主 Agent 首次最终回复
    /// - `on_event`: 流式事件回调
    /// - `perf`: 性能追踪器
    ///
    /// 返回:
    /// - 合并主动消费补充回复后的最终结果
    pub(super) async fn consume_subagent_results_after_final<F>(
        &mut self,
        turn_id: &str,
        messages: &mut Vec<ChatMessage>,
        initial_result: ChatResult,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        if !self.tools.contains("subagent") {
            return Ok(initial_result);
        }
        let owner_key = self.state.state_dir().display().to_string();
        let mut combined = initial_result;
        messages.push(ChatMessage::assistant(combined.content.clone(), None));

        loop {
            // 1. 【Sai】【主动消费】等待本轮后台子智能体结束,持久化未消费列表在请求失败时仍会保留结果
            while pending_finished_notices(&owner_key).is_empty() {
                let subagents = list_subagents_for_owner(&owner_key);
                if subagents
                    .iter()
                    .all(|snapshot| snapshot.status != "running")
                {
                    // 2. 【Sai】【主动消费】终态可能出现在两次读取之间,返回前再次确认未消费列表
                    if pending_finished_notices(&owner_key).is_empty() {
                        return Ok(combined);
                    }
                    break;
                }
                tokio::time::sleep(COMPLETION_POLL_INTERVAL).await;
            }

            // 3. 【Sai】【主动消费】把未消费结果作为系统提醒提交给同一主模型,本次请求禁止工具调用
            let mut reminder = SubagentReminder::new(owner_key.clone());
            let Some(content) = reminder.after_tool_round() else {
                continue;
            };
            messages.push(ChatMessage::system(content));
            let ordered_messages = system_messages_first(messages.clone());
            let projection =
                project_provider_turn_from_messages(&ordered_messages, 0, self.context_char_budget);
            self.state
                .enforce_provider_projection(Some(turn_id), &projection)?;
            perf.mark("active subagent result consumption start");
            let result = self
                .client
                .chat_stream_events(ordered_messages, Vec::new(), |event| match event {
                    ChatStreamEvent::Chunk(chunk) => on_event(AgentEvent::Chunk(chunk)),
                    ChatStreamEvent::ToolCallProgress(_) => Ok(()),
                })
                .await?;
            reminder.acknowledge_delivered();
            messages.pop();
            messages.push(ChatMessage::assistant(result.content.clone(), None));
            append_result(&mut combined, result);
            perf.mark("active subagent result consumption done");
        }
    }
}

/// 合并主回复与主动消费子智能体结果产生的补充回复。
///
/// 参数:
/// - `target`: 已累计的聊天结果
/// - `addition`: 本次补充消费结果
///
/// 返回:
/// - 无
fn append_result(target: &mut ChatResult, addition: ChatResult) {
    // 1. 【Sai】【主动消费】正文和思考内容按模型实际输出顺序拼接
    append_text(&mut target.content, &addition.content);
    if let Some(reasoning) = addition.reasoning {
        let target_reasoning = target.reasoning.get_or_insert_with(String::new);
        append_text(target_reasoning, &reasoning);
    }
    // 2. 【Sai】【主动消费】用量按所有主模型请求累计
    target.usage = merge_usage(target.usage.take(), addition.usage);
    target.tool_calls.clear();
}

/// 按段落边界追加非空文本。
///
/// 参数:
/// - `target`: 目标文本
/// - `addition`: 待追加文本
///
/// 返回:
/// - 无
fn append_text(target: &mut String, addition: &str) {
    if addition.trim().is_empty() {
        return;
    }
    if !target.trim().is_empty() {
        target.push_str("\n\n");
    }
    target.push_str(addition);
}

/// 合并多次主模型请求的 token 用量。
///
/// 参数:
/// - `left`: 已累计用量
/// - `right`: 新增用量
///
/// 返回:
/// - 合并后的可选用量
fn merge_usage(left: Option<Usage>, right: Option<Usage>) -> Option<Usage> {
    match (left, right) {
        (None, None) => None,
        (left, right) => {
            let left = left.unwrap_or_default();
            let right = right.unwrap_or_default();
            Some(Usage {
                prompt_tokens: left.prompt_tokens + right.prompt_tokens,
                completion_tokens: left.completion_tokens + right.completion_tokens,
                total_tokens: left.total_tokens + right.total_tokens,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证主动消费产生的正文、思考与用量可以合并。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn appends_completion_content_and_usage() {
        let mut target = ChatResult {
            content: "初始回复".to_string(),
            reasoning: None,
            usage: Some(Usage {
                prompt_tokens: 2,
                completion_tokens: 3,
                total_tokens: 5,
            }),
            tool_calls: Vec::new(),
        };
        append_result(
            &mut target,
            ChatResult {
                content: "补充结果".to_string(),
                reasoning: Some("补充思考".to_string()),
                usage: Some(Usage {
                    prompt_tokens: 7,
                    completion_tokens: 11,
                    total_tokens: 18,
                }),
                tool_calls: Vec::new(),
            },
        );

        assert_eq!(target.content, "初始回复\n\n补充结果");
        assert_eq!(target.reasoning.as_deref(), Some("补充思考"));
        assert_eq!(target.usage.unwrap().total_tokens, 23);
    }
}
