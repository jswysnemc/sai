use crate::goal::{Goal, GoalStatus};
use crate::i18n::text as t;
use crate::state::StateStore;
use anyhow::{bail, Result};

mod arguments;
use arguments::parse_objective_and_budget;

/// `/goal` 子命令。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GoalCommand {
    Show,
    Set {
        objective: String,
        token_budget: Option<u64>,
    },
    Edit {
        objective: String,
    },
    Pause,
    Resume,
    Clear,
}

/// `/goal` 执行结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalCommandOutcome {
    pub message: String,
    pub should_continue: bool,
}

/// 解析 `/goal` 参数文本。
///
/// 参数:
/// - `input`: `/goal` 后的参数
///
/// 返回:
/// - Goal 子命令
pub(super) fn parse_goal_command(input: &str) -> Result<GoalCommand> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(GoalCommand::Show);
    }
    if input.eq_ignore_ascii_case("pause") {
        return Ok(GoalCommand::Pause);
    }
    if input.eq_ignore_ascii_case("resume") {
        return Ok(GoalCommand::Resume);
    }
    if input.eq_ignore_ascii_case("clear") {
        return Ok(GoalCommand::Clear);
    }
    if let Some(objective) = input.strip_prefix("edit ") {
        let objective = objective.trim();
        if objective.is_empty() {
            bail!("usage: /goal edit <objective>")
        }
        return Ok(GoalCommand::Edit {
            objective: objective.to_string(),
        });
    }
    let (objective, token_budget) = parse_objective_and_budget(input)?;
    Ok(GoalCommand::Set {
        objective,
        token_budget,
    })
}

/// 执行 Goal 状态命令。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `command`: Goal 子命令
///
/// 返回:
/// - 展示文本和是否需要启动自动续轮
pub fn execute_goal_command(
    state: &StateStore,
    command: GoalCommand,
) -> Result<GoalCommandOutcome> {
    match command {
        GoalCommand::Show => Ok(GoalCommandOutcome {
            message: match state.goal()? {
                Some(goal) => goal_summary(&goal),
                None => t(
                    "No goal is set. Usage: /goal <objective> [--tokens N]",
                    "尚未设置目标。用法：/goal <目标> [--tokens N]",
                )
                .to_string(),
            },
            should_continue: false,
        }),
        GoalCommand::Set {
            objective,
            token_budget,
        } => {
            let goal = state.replace_goal(&objective, token_budget, false)?;
            Ok(GoalCommandOutcome {
                message: goal_summary(&goal),
                should_continue: true,
            })
        }
        GoalCommand::Edit { objective } => {
            let goal = state.edit_goal(&objective)?;
            Ok(GoalCommandOutcome {
                message: goal_summary(&goal),
                should_continue: goal.status.is_active(),
            })
        }
        GoalCommand::Pause => status_outcome(state, GoalStatus::Paused, false),
        GoalCommand::Resume => status_outcome(state, GoalStatus::Active, true),
        GoalCommand::Clear => {
            let cleared = state.clear_goal()?;
            Ok(GoalCommandOutcome {
                message: if cleared {
                    t("Goal cleared", "目标已清除").to_string()
                } else {
                    t("No goal is set", "尚未设置目标").to_string()
                },
                should_continue: false,
            })
        }
    }
}

/// 更新目标状态并构造命令结果。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `status`: 新状态
/// - `continue_when_active`: 活动状态时是否续轮
///
/// 返回:
/// - Goal 命令结果
fn status_outcome(
    state: &StateStore,
    status: GoalStatus,
    continue_when_active: bool,
) -> Result<GoalCommandOutcome> {
    let goal = state.set_goal_status(status)?;
    Ok(GoalCommandOutcome {
        message: goal_summary(&goal),
        should_continue: continue_when_active && goal.status.is_active(),
    })
}

/// 格式化 Goal 状态摘要。
///
/// 参数:
/// - `goal`: 当前目标
///
/// 返回:
/// - 适合终端和渠道展示的摘要
fn goal_summary(goal: &Goal) -> String {
    let budget = goal
        .token_budget
        .map(|value| value.to_string())
        .unwrap_or_else(|| t("unlimited", "不限").to_string());
    format!(
        "{}: {}\n{}: {}\nToken: {} / {}\n{}: {}s",
        t("Goal", "目标"),
        goal.objective,
        t("Status", "状态"),
        goal.status.as_str(),
        goal.tokens_used,
        budget,
        t("Time used", "已用时间"),
        goal.time_used_seconds,
    )
}
