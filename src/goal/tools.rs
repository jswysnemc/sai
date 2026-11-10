use super::{GoalStatus, GoalStore};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

/// 注册会话目标工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `goal_file`: 当前会话目标文件
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry, goal_file: PathBuf) {
    let create_store = GoalStore::new(goal_file.clone());
    registry.register(ToolSpec::new(
        "create_goal",
        "Create a goal only when explicitly requested by the user or system/developer instructions; do not infer goals from ordinary tasks. Set token_budget only when explicitly requested. Fails if an unfinished goal exists; use update_goal only for status.",
        json!({
            "type": "object",
            "properties": {
                "objective": {"type": "string", "description": "Concrete objective to pursue."},
                "token_budget": {"type": "integer", "minimum": 1, "description": "Optional positive token budget."}
            },
            "required": ["objective"],
            "additionalProperties": false
        }),
        move |args| {
            let store = create_store.clone();
            async move { create_goal(&store, &args) }
        },
    ).writes());

    let get_store = GoalStore::new(goal_file.clone());
    registry.register(ToolSpec::new(
        "get_goal",
        "Read the current persistent goal, including status, budget, token usage, and elapsed time.",
        json!({"type":"object","properties":{},"additionalProperties":false}),
        move |_| {
            let store = get_store.clone();
            async move { get_goal(&store) }
        },
    ));

    let update_store = GoalStore::new(goal_file);
    registry.register(
        ToolSpec::new(
            "update_goal",
            super::instructions::UPDATE_GOAL,
            json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string", "enum": ["complete", "blocked", "paused"]}

                },
                "required": ["status"],
                "additionalProperties": false
            }),
            move |args| {
                let store = update_store.clone();
                async move { update_goal(&store, &args) }
            },
        )
        .writes(),
    );
}

/// 创建模型请求的持续目标。
///
/// 参数:
/// - `store`: 当前会话目标存储
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 格式目标
fn create_goal(store: &GoalStore, args: &Value) -> Result<String> {
    let objective = required_string(args, "objective")?;
    let token_budget = optional_u64(args, "token_budget")?;
    let goal = store.replace(&objective, token_budget, false)?;
    goal_response(Some(goal), false)
}

/// 读取当前持续目标。
///
/// 参数:
/// - `store`: 当前会话目标存储
///
/// 返回:
/// - JSON 格式目标或空目标结果
fn get_goal(store: &GoalStore) -> Result<String> {
    goal_response(store.get()?, false)
}

/// 【目标】【工具结果】返回目标、剩余预算及完成时的用量报告，隐藏内部核算水位
/// 参数：目标快照、是否完成；返回 JSON 工具结果
fn goal_response(goal: Option<super::Goal>, complete: bool) -> Result<String> {
    let remaining = goal.as_ref().and_then(|goal| {
        goal.token_budget
            .map(|budget| budget.saturating_sub(goal.tokens_used))
    });
    let report = goal
        .as_ref()
        .filter(|goal| complete && goal.status == GoalStatus::Complete)
        .and_then(|goal| {
            goal.token_budget.map(|budget| {
                format!(
                    "Goal completed using {} of {budget} tokens",
                    goal.tokens_used
                )
            })
        });
    let mut value = serde_json::to_value(&goal)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("usage_sources");
    }
    Ok(serde_json::to_string_pretty(
        &json!({"goal": value, "remainingTokens": remaining, "completionBudgetReport": report}),
    )?)
}

/// 更新模型请求的目标终态。
///
/// 参数:
/// - `store`: 当前会话目标存储
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 格式更新结果
fn update_goal(store: &GoalStore, args: &Value) -> Result<String> {
    let status = required_string(args, "status")?;
    let status = match status.as_str() {
        "complete" => GoalStatus::Complete,
        "blocked" => GoalStatus::Blocked,
        "paused" => GoalStatus::Paused,
        _ => bail!("update_goal status must be complete, blocked or paused"),
    };
    let goal = store.set_status(status)?;
    goal_response(Some(goal), status == GoalStatus::Complete)
}

/// 读取必填非空字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `name`: 参数名称
///
/// 返回:
/// - 归一化字符串
fn required_string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

/// 读取可选正整数参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `name`: 参数名称
///
/// 返回:
/// - 可选整数
fn optional_u64(args: &Value, name: &str) -> Result<Option<u64>> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("{name} must be a positive integer"))?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registers_and_executes_goal_tools() {
        let temp = tempfile::tempdir().unwrap();
        let goal_file = temp.path().join("goal.json");
        let mut registry = ToolRegistry::new();
        register(&mut registry, goal_file.clone());

        assert!(registry.contains("create_goal"));
        assert!(registry.contains("get_goal"));
        assert!(registry.contains("update_goal"));
        let update = registry
            .tool_infos()
            .into_iter()
            .find(|tool| tool.name == "update_goal")
            .unwrap();
        assert!(update.description.contains("user's explicit request"));
        assert!(update.description.contains("three consecutive goal turns"));

        registry
            .call(
                "create_goal",
                r#"{"objective":"finish validation","token_budget":1000}"#,
            )
            .await
            .unwrap();
        registry
            .call("update_goal", r#"{"status":"complete"}"#)
            .await
            .unwrap();

        let goal = GoalStore::new(goal_file).get().unwrap().unwrap();
        assert_eq!(goal.status, GoalStatus::Complete);
    }
}
