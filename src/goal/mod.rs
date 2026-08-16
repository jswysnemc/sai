mod model;
mod prompt;
mod store;
mod tools;

use crate::config::AppConfig;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::path::PathBuf;

pub(crate) use model::{Goal, GoalStatus, GoalUpdateEntry};
pub(crate) use prompt::{continuation_prompt, is_continuation_input, system_context};
pub(crate) use store::GoalStore;

/// 按当前 Agent 工具策略注册会话目标工具。
///
/// 独占白名单必须是最终工具集，因此只补入其中显式列出的目标工具；
/// 兼容模式继续保留原有的隐式目标工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `goal_file`: 当前会话目标文件
/// - `config`: 已应用 Agent 覆盖的运行配置
///
/// 返回:
/// - 工具复制结果
pub(crate) fn register_tools_for_config(
    registry: &mut ToolRegistry,
    goal_file: PathBuf,
    config: &AppConfig,
) -> Result<()> {
    let exclusive = config
        .agent_runtime
        .as_ref()
        .is_some_and(|runtime| runtime.exclusive);
    if !exclusive {
        tools::register(registry, goal_file);
        return Ok(());
    }

    // 1. 在临时注册表构造完整目标工具，避免复制未授权项目
    let mut candidates = ToolRegistry::new();
    tools::register(&mut candidates, goal_file);
    // 2. 独占模式只复制白名单明确允许的目标工具
    for name in ["create_goal", "get_goal", "update_goal"] {
        if crate::config::whitelist_allows_tool(config, name) {
            registry.register_from(&candidates, name)?;
        }
    }
    Ok(())
}
