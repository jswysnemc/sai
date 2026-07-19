use crate::goal::Goal;

/// 自动输入的来源类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutomaticInputKind {
    GoalContinuation,
    ExternalCompletion,
}

impl AutomaticInputKind {
    /// 返回稳定的事件类型文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 自动输入来源标识
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::GoalContinuation => "goal_continuation",
            Self::ExternalCompletion => "external_completion",
        }
    }
}

/// 等待进入 Agent 的自动输入队列项。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AutomaticInput {
    pub(crate) kind: AutomaticInputKind,
    pub(crate) prompt: Option<String>,
    pub(crate) display: Option<String>,
}

impl AutomaticInput {
    /// 创建普通 Goal 自动续轮队列项。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - Goal 自动续轮输入
    pub(crate) fn goal_continuation() -> Self {
        Self {
            kind: AutomaticInputKind::GoalContinuation,
            prompt: None,
            display: None,
        }
    }

    /// 创建带外部完成结果的自动续轮队列项。
    ///
    /// 参数:
    /// - `prompt`: 发送给模型的完整外部事件提示
    /// - `display`: 展示给用户的完成结果摘要
    ///
    /// 返回:
    /// - 外部完成自动续轮输入
    pub(crate) fn external_completion(prompt: String, display: String) -> Self {
        Self {
            kind: AutomaticInputKind::ExternalCompletion,
            prompt: Some(prompt),
            display: Some(display),
        }
    }

    /// 返回展示给用户的自动消息文本。
    ///
    /// 参数:
    /// - `goal`: 当前 Goal
    ///
    /// 返回:
    /// - 简洁的自动消息文本
    pub(crate) fn display_text(&self, goal: &Goal) -> String {
        self.display
            .clone()
            .unwrap_or_else(|| format!("Goal 自动续轮：{}", goal.objective))
    }
}

/// 已开始发送给 Agent 的自动输入事件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AutomaticInputEvent {
    pub(crate) kind: AutomaticInputKind,
    pub(crate) content: String,
}

impl AutomaticInputEvent {
    /// 创建自动输入事件。
    ///
    /// 参数:
    /// - `kind`: 自动输入来源
    /// - `content`: 展示给用户的文本
    ///
    /// 返回:
    /// - 自动输入事件
    pub(crate) fn new(kind: AutomaticInputKind, content: String) -> Self {
        Self { kind, content }
    }
}
