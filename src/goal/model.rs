use serde::{Deserialize, Serialize};

/// 会话持续目标状态。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

impl GoalStatus {
    /// 返回稳定状态文本。
    ///
    /// 返回:
    /// - snake_case 状态文本
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::UsageLimited => "usage_limited",
            Self::BudgetLimited => "budget_limited",
            Self::Complete => "complete",
        }
    }

    /// 判断状态是否允许自动续轮。
    ///
    /// 返回:
    /// - 是否为活动状态
    pub(crate) fn is_active(self) -> bool {
        self == Self::Active
    }

    /// 判断状态是否已经结束当前目标。
    ///
    /// 返回:
    /// - 是否为完成或预算终止状态
    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::BudgetLimited | Self::Complete)
    }

    /// 从 API 或工具参数解析状态。
    ///
    /// 参数:
    /// - `value`: 状态文本
    ///
    /// 返回:
    /// - 已识别状态
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "active" | "resume" => Ok(Self::Active),
            "paused" | "pause" => Ok(Self::Paused),
            "blocked" => Ok(Self::Blocked),
            "usage_limited" => Ok(Self::UsageLimited),
            "budget_limited" => Ok(Self::BudgetLimited),
            "complete" | "completed" => Ok(Self::Complete),
            other => anyhow::bail!("unknown goal status: {other}"),
        }
    }
}

/// 会话级持续执行目标。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Goal {
    pub id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: String,
    pub updated_at: String,
}
