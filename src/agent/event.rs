use crate::llm::{ChatStreamChunk, ToolCallStreamProgress, Usage};

/// 一次模型消息完成后的上下文用量更新。
#[derive(Debug, Clone)]
pub struct MessageContextUpdate {
    pub usage: Option<Usage>,
    pub context_window_tokens: usize,
}

/// 上下文压缩失败的用户可见信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionError {
    pub message: String,
    pub detail: String,
}

/// Agent 向 CLI、TUI 与 Web 发送的统一运行事件。
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// 撤销失败请求的部分输出；计数使用 Unicode 字符，不影响之前完成的请求
    ResponseReset {
        content_chars: usize,
        reasoning_chars: usize,
        compaction: bool,
    },
    Chunk(ChatStreamChunk),
    /// 当前模型回合中插入的完成回执、Goal 续作或排队用户消息。
    InterMessage(super::InterMessageEvent),
    /// 当前模型回合正在等待后台工作产生下一条消息。
    WaitingExternal,
    /// 传输层瞬时故障后正在自动重连。
    ///
    /// 失败尝试的部分输出先经 ResponseReset 撤销，再展示重连次数
    Reconnecting {
        attempt: u32,
        max_attempts: u32,
    },
    /// 当前 provider 消息已经完成，上下文统计可以立即刷新。
    ContextUpdated(MessageContextUpdate),
    ToolCall {
        name: String,
        arguments: String,
    },
    /// 带 provider 稳定标识的工具调用。
    ///
    /// 工具结果可能省略名称；保留调用标识后，
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
    /// 用户消息发往模型前的 Jev 预选。
    ///
    /// 只通知界面。不写入助手工具调用，避免思考型模型缺少 reasoning。
    JevPreselect {
        /// running、ready、empty 或 failed。
        phase: String,
        /// ready 时为暴露结果 JSON，failed 时为错误摘要。
        detail: String,
    },
}
