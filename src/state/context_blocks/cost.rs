use super::model::{Block, CompressRequest};
use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};
use anyhow::Result;

/// 【上下文】【净收益预检】估算新增摘要调用和简短回执的消息开销
/// 参数: request 为模型摘要参数，block 为拟提交块；返回两条新消息的 token 估算
pub(super) fn submission_tokens(request: &CompressRequest, block: &Block) -> Result<usize> {
    let call_id = "context_compression_submission";
    let messages = [
        ChatMessage::assistant(
            "",
            Some(vec![ToolCall {
                id: call_id.into(),
                kind: "function".into(),
                function: ToolCallFunction {
                    name: "compress_context".into(),
                    arguments: serde_json::to_string(request)?,
                },
            }]),
        ),
        ChatMessage::tool(call_id, serde_json::to_string(&block.receipt())?),
    ];
    Ok(crate::state::compaction::estimate_chat_messages_tokens(
        &messages,
    ))
}
