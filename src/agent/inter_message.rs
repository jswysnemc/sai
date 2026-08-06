use anyhow::Result;

/// 模型消息间隙可以插入的消息类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterMessageKind {
    GoalContinuation,
    ExternalCompletion,
    QueuedUser,
}

impl InterMessageKind {
    /// 返回事件与持久化共同使用的稳定类型文本。
    ///
    /// 返回:
    /// - 类型文本
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::GoalContinuation => "goal_continuation",
            Self::ExternalCompletion => "external_completion",
            Self::QueuedUser => "queued_user",
        }
    }
}

/// 等待插入当前模型回合的消息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InterMessage {
    pub(crate) id: String,
    pub(crate) kind: InterMessageKind,
    pub(crate) prompt: String,
    pub(crate) display: String,
    pub(crate) image_urls: Vec<String>,
}

impl InterMessage {
    /// 创建一条用户排队消息。
    ///
    /// 参数:
    /// - `id`: 来源稳定标识
    /// - `content`: 用户消息正文
    /// - `image_urls`: 图片附件
    ///
    /// 返回:
    /// - 可插入模型间隙的消息
    pub(crate) fn queued_user(
        id: impl Into<String>,
        content: impl Into<String>,
        image_urls: Vec<String>,
    ) -> Self {
        let content = content.into();
        Self {
            id: id.into(),
            kind: InterMessageKind::QueuedUser,
            prompt: content.clone(),
            display: content,
            image_urls,
        }
    }
}

/// Agent 向界面发送的模型间隙消息事件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InterMessageEvent {
    pub(crate) id: String,
    pub(crate) kind: InterMessageKind,
    pub(crate) content: String,
}

/// 为活动模型回合提供可编辑排队消息的来源。
#[async_trait::async_trait]
pub(crate) trait InterMessageSource: Send + Sync {
    /// 查看下一条消息但不确认消费。
    ///
    /// 返回:
    /// - 当前队首消息；队列为空时返回空
    async fn peek(&self) -> Result<Option<InterMessage>>;

    /// 确认消息已经包含在一次成功的模型请求中。
    ///
    /// 参数:
    /// - `message_id`: 已交付消息标识
    ///
    /// 返回:
    /// - 确认是否成功
    async fn acknowledge(&self, message_id: &str) -> Result<()>;
}
