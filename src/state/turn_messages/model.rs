/// 同一持久化轮次中的消息类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TurnMessageKind {
    Assistant,
    ExternalCompletion,
    GoalContinuation,
    QueuedUser,
}

impl TurnMessageKind {
    /// 返回数据库使用的稳定类型文本。
    ///
    /// 返回:
    /// - 类型文本
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::ExternalCompletion => "external_completion",
            Self::GoalContinuation => "goal_continuation",
            Self::QueuedUser => "queued_user",
        }
    }

    /// 从数据库文本恢复消息类型。
    ///
    /// 参数:
    /// - `value`: 数据库类型文本
    ///
    /// 返回:
    /// - 消息类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "external_completion" => Self::ExternalCompletion,
            "goal_continuation" => Self::GoalContinuation,
            "queued_user" => Self::QueuedUser,
            _ => Self::Assistant,
        }
    }

    /// 返回发送给模型时使用的消息角色。
    ///
    /// 返回:
    /// - `assistant` 或 `user`
    pub(crate) fn role(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::ExternalCompletion | Self::GoalContinuation | Self::QueuedUser => "user",
        }
    }
}

/// 待写入的轮次内消息。
#[derive(Clone, Debug)]
pub(crate) struct NewTurnMessage {
    pub(crate) turn_id: String,
    pub(crate) after_tool_seq: usize,
    pub(crate) kind: TurnMessageKind,
    pub(crate) model_content: String,
    pub(crate) display_content: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) image_urls: Vec<String>,
}

/// 已持久化的轮次内消息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TurnMessageRecord {
    pub(crate) id: String,
    pub(crate) turn_id: String,
    pub(crate) seq: usize,
    pub(crate) after_tool_seq: usize,
    pub(crate) kind: TurnMessageKind,
    pub(crate) model_content: String,
    pub(crate) display_content: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) image_urls: Vec<String>,
    pub(crate) created_at: String,
}
