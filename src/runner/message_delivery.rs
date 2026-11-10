use crate::goal::GoalStatus;
use serde::{Deserialize, Serialize};

/// 【消息队列】【投递时机】统一终端、网页和持久化队列的发送语义
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueueInsertAt {
    /// 在下一次工具间隙立即投递，空闲时直接开始处理
    #[default]
    Request,
    /// 整体任务完成后发送；存在未完成目标时跨自动续轮等待
    Turn,
}

impl QueueInsertAt {
    /// 【消息队列】【时机编辑】切换当前消息的发送时机
    /// 参数：无；返回：切换后的时机
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Request => Self::Turn,
            Self::Turn => Self::Request,
        }
    }

    /// 【消息队列】【空闲投递】判定消息是否可以在当前任务边界发送
    /// 参数：`goal_status` 为当前目标状态；返回：是否允许开始处理消息
    pub(crate) fn ready_when_idle(self, goal_status: Option<GoalStatus>) -> bool {
        self == Self::Request || goal_status.is_none_or(|status| status == GoalStatus::Complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【消息队列】【默认测试】普通提交默认立即发送并兼容已有序列化值；无参数、无返回值
    #[test]
    fn default_is_immediate_and_wire_values_remain_compatible() {
        assert_eq!(QueueInsertAt::default(), QueueInsertAt::Request);
        assert_eq!(
            serde_json::from_str::<QueueInsertAt>("\"turn\"").unwrap(),
            QueueInsertAt::Turn
        );
    }

    /// 【消息队列】【完成测试】暂停、阻塞和预算限制都不等于任务完成；无参数、无返回值
    #[test]
    fn deferred_messages_wait_for_goal_completion() {
        for status in [
            GoalStatus::Active,
            GoalStatus::Paused,
            GoalStatus::Blocked,
            GoalStatus::UsageLimited,
            GoalStatus::BudgetLimited,
        ] {
            assert!(!QueueInsertAt::Turn.ready_when_idle(Some(status)));
            assert!(QueueInsertAt::Request.ready_when_idle(Some(status)));
        }
        assert!(QueueInsertAt::Turn.ready_when_idle(Some(GoalStatus::Complete)));
        assert!(QueueInsertAt::Turn.ready_when_idle(None));
    }
}
