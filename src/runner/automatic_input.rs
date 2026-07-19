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
    pub(crate) fn display_text(&self, goal: Option<&Goal>) -> String {
        self.display
            .clone()
            .unwrap_or_else(|| {
                goal.map(|goal| format!("Goal 自动续轮：{}", goal.objective))
                    .unwrap_or_else(|| "自动继续当前任务".to_string())
            })
    }

    /// 构造自动队列项发送给模型的用户输入。
    ///
    /// 参数:
    /// - `goal`: 当前活动 Goal，可用于构造续轮提示
    ///
    /// 返回:
    /// - 用户输入文本；Goal 续轮缺少活动目标时返回空值
    pub(crate) fn prompt_text(&self, goal: Option<&Goal>) -> Option<String> {
        let mut prompt = match self.kind {
            AutomaticInputKind::GoalContinuation => {
                crate::goal::continuation_prompt(goal?)
            }
            AutomaticInputKind::ExternalCompletion => goal
                .map(crate::goal::continuation_prompt)
                .unwrap_or_default(),
        };
        if let Some(extra) = &self.prompt {
            if !prompt.is_empty() {
                prompt.push_str("\n\n");
            }
            prompt.push_str(extra);
        }
        Some(prompt)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_completion_can_build_a_non_goal_prompt() {
        let input = AutomaticInput::external_completion(
            "<external-completion-events>done</external-completion-events>".to_string(),
            "后台工作已完成".to_string(),
        );

        assert_eq!(
            input.prompt_text(None).as_deref(),
            Some("<external-completion-events>done</external-completion-events>")
        );
        assert_eq!(input.display_text(None), "后台工作已完成");
    }
}
