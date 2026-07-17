use super::model::{ProjectedRequest, ProjectionEstimate};
use crate::llm::ChatMessage;
use crate::state::compaction::{estimate_chat_messages_chars, estimate_chat_messages_tokens};
use crate::state::session_snapshot;

/// 估算 provider turn 投影视图。
///
/// 参数:
/// - `messages`: 当前请求消息列表
/// - `context_limit_chars`: 当前模型上下文窗口（token 口径，历史字段名保留）
///
/// 返回:
/// - provider 请求估算；`message_chars` 实际存 token 数，供压缩阈值比较
pub(crate) fn project_provider_turn_estimate(
    messages: &[ChatMessage],
    context_limit_chars: usize,
) -> ProjectionEstimate {
    let message_tokens = estimate_chat_messages_tokens(messages);
    let message_chars = estimate_chat_messages_chars(messages);
    ProjectionEstimate {
        // 压缩路径比较的是 message_chars 与 context_limit；此处写入 token 估算，
        // 与 Agent 使用的 window tokens 对齐。
        message_chars: message_tokens,
        state_context_chars: message_chars,
        context_limit_chars,
        context_ratio: session_snapshot::context_ratio(message_tokens, context_limit_chars),
    }
}

/// 读取投影请求的 provider 上下文占用估算。
///
/// 参数:
/// - `projection`: provider 请求投影视图
///
/// 返回:
/// - provider 上下文占用估算（token）
pub(crate) fn estimate_projected_request_chars(projection: &ProjectedRequest) -> usize {
    projection.estimate.message_chars
}
