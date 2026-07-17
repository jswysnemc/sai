use super::background_tasks::{
    cleanup_background_tasks, list_background_tasks, read_background_task_output,
    start_background_task, stop_background_task, BackgroundRuntimeOwner,
};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::Value;

/// 执行后台命令统一 action。
///
/// 参数:
/// - `args`: 工具参数
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allow_command_execution`: 是否允许命令执行
/// - `readonly`: 是否为只读工具注册
///
/// 返回:
/// - JSON 格式执行结果
pub(super) async fn run_background_action(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
    allow_command_execution: bool,
    readonly: bool,
    runtime_owner: Option<BackgroundRuntimeOwner>,
) -> Result<String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    match action {
        "start" if !readonly => {
            start_background_task(args, config, paths, allow_command_execution, runtime_owner)
        }
        "list" => list_background_tasks(paths, config).await,
        "output" => read_background_task_output(args, config, paths).await,
        "stop" if !readonly => stop_background_task(args, config, paths).await,
        "cleanup" if !readonly => cleanup_background_tasks(args, paths, config).await,
        "start" | "stop" | "cleanup" => bail!(
            "{}",
            t(
                "background_command read-only mode only supports action=list and action=output",
                "background_command 只读模式仅支持 action=list 和 action=output"
            )
        ),
        _ => bail!(
            "{}: {action}",
            t("unknown background command action", "未知后台命令操作")
        ),
    }
}
