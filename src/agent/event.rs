use crate::llm::{ChatStreamChunk, ToolCallStreamProgress};

/// 上下文压缩失败的用户可见信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionError {
    pub message: String,
    pub detail: String,
}

/// Agent 向 CLI、TUI 与 Web 发送的统一运行事件。
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Chunk(ChatStreamChunk),
    ToolCall {
        name: String,
        arguments: String,
    },
    ToolCallProgress(ToolCallStreamProgress),
    ToolResult {
        name: String,
        ok: bool,
        output: String,
    },
    ToolProgress {
        name: String,
        message: String,
    },
    PermissionRequested(crate::permission::PermissionRequest),
    PermissionResolved {
        request_id: String,
        decision: crate::permission::PermissionDecision,
    },
    QuestionRequested(crate::question::PendingQuestion),
    QuestionResolved {
        request_id: String,
        response: crate::question::QuestionResponse,
    },
    CompactionStarted {
        turn_count: usize,
        model: String,
    },
    CompactionDelta {
        text: String,
    },
    CompactionFinished {
        applied: bool,
        /// 成功应用时的压缩摘要正文；未应用时为空。
        summary: Option<String>,
        /// 失败时的概要与可展开详情。
        error: Option<CompactionError>,
    },
    FlushContent,
    ExternalOutput,
}
