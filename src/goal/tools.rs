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
        "Create a persistent active goal only when the user explicitly asks for ongoing autonomous work. Fails while an unfinished goal exists. Set token_budget only when the user provided a budget.",
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
    ));

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
    registry.register(ToolSpec::new(
        "update_goal",
        "Set the current goal to complete only after the full objective is verified, or blocked only after the same blocking condition repeats for at least three consecutive goal turns and progress is impossible without user input or an external change. Do not use this tool merely to pause or limit work.",
        json!({
            "type": "object",
            "properties": {
                "status": {"type": "string", "enum": ["complete", "blocked"]}
            },
            "required": ["status"],
            "additionalProperties": false
        }),
        move |args| {
            let store = update_store.clone();
            async move { update_goal(&store, &args) }
        },
    ));
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
    Ok(serde_json::to_string_pretty(&goal)?)
}

/// 读取当前持续目标。
///
/// 参数:
/// - `store`: 当前会话目标存储
///
/// 返回:
/// - JSON 格式目标或空目标结果
fn get_goal(store: &GoalStore) -> Result<String> {
    Ok(serde_json::to_string_pretty(&json!({
        "goal": store.get()?
    }))?)
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
    let status = match status.to_ascii_lowercase().as_str() {
        "complete" => GoalStatus::Complete,
        "blocked" => GoalStatus::Blocked,
        _ => bail!("update_goal status must be complete or blocked"),
    };
    let goal = store.set_status(status)?;
    Ok(serde_json::to_string_pretty(&goal)?)
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
