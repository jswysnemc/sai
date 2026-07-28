use super::TranscriptRenderOptions;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【测试辅助】创建 transcript 测试使用的渲染选项。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 展示完整思考与工具摘要的渲染选项
pub(super) fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 【终端】【测试辅助】创建指定类型的流式文本分片。
///
/// 参数:
/// - `kind`: 分片类型
/// - `text`: 分片正文
///
/// 返回:
/// - transcript 可接收的流式分片
pub(super) fn chunk(kind: ChatStreamKind, text: &str) -> ChatStreamChunk {
    ChatStreamChunk {
        kind,
        text: text.to_string(),
    }
}
