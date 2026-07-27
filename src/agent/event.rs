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
    /// 带 provider 稳定标识的工具调用。
    ///
    /// ACP 的更新可能省略名称，但始终携带 `toolCallId`。保留该标识后，
    /// Web 与历史存储不再依赖不稳定的标题关联生命周期。
    ToolCallIdentified {
        id: String,
        name: String,
        arguments: String,
    },
    ToolCallProgress(ToolCallStreamProgress),
    ToolResult {
        name: String,
        ok: bool,
        output: String,
    },
    /// 带 provider 稳定标识的工具结果。
    ToolResultIdentified {
        id: String,
        name: String,
        ok: bool,
        output: String,
    },
    ToolProgress {
        name: String,
        message: String,
    },
    /// 带 provider 稳定标识的工具进度。
    ToolProgressIdentified {
        id: String,
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
    /// 外部对话内核已连接。
    ///
    /// 名称与版本来自 ACP 握手响应里的 agentInfo，只有真正连上子进程才拿得到，
    /// 因此这是「本轮由谁执行」的运行时证据，而不是配置读数的复述。
    EngineReady {
        engine: String,
        version: String,
    },
}
