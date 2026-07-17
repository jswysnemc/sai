use super::background_actions::run_background_action;
use super::background_schema::{
    background_tool_name, readonly_description, readonly_schema, writable_description,
    writable_schema,
};
use super::background_tasks::{
    cleanup_background_tasks, list_background_tasks, read_background_task_output,
    start_background_task, stop_background_task, BackgroundRuntimeOwner,
};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::json;

/// 注册后台命令写入类工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allow_command_execution`: 是否允许命令执行
pub(crate) fn register(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    allow_command_execution: bool,
) {
    register_with_runtime_owner(registry, config, paths, allow_command_execution, None);
}

/// 注册带运行时 owner 的后台命令写入类工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allow_command_execution`: 是否允许命令执行
/// - `runtime_owner`: 可选运行时 owner 元数据
pub(crate) fn register_with_runtime_owner(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    allow_command_execution: bool,
    runtime_owner: Option<BackgroundRuntimeOwner>,
) {
    let action_config = config.clone();
    let action_paths = paths.clone();
    let action_owner = runtime_owner.clone();
    registry.register(
        ToolSpec::new(
            background_tool_name(),
            writable_description(),
            writable_schema(),
            move |args| {
                let config = action_config.clone();
                let paths = action_paths.clone();
                let runtime_owner = action_owner.clone();
                async move {
                    run_background_action(
                        args,
                        &config,
                        &paths,
                        allow_command_execution,
                        false,
                        runtime_owner,
                    )
                    .await
                }
            },
        )
        .writes(),
    );
}

/// 注册后台命令只读工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
pub(crate) fn register_readonly(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    let action_config = config.clone();
    let action_paths = paths.clone();
    registry.register(ToolSpec::new(
        background_tool_name(),
        readonly_description(),
        readonly_schema(),
        move |args| {
            let config = action_config.clone();
            let paths = action_paths.clone();
            async move { run_background_action(args, &config, &paths, false, true, None).await }
        },
    ));
}

/// 用户 CLI 列出后台命令。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 格式任务列表
pub(crate) async fn list_background_tasks_for_user(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<String> {
    list_background_tasks(paths, config).await
}

/// 用户 CLI 启动后台命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `command`: shell 命令
/// - `cwd`: 可选工作目录
/// - `label`: 可选标签
/// - `timeout_seconds`: 可选超时时间，0 表示不自动超时
///
/// 返回:
/// - JSON 格式任务信息
pub(crate) fn start_background_task_for_user(
    paths: &SaiPaths,
    config: &AppConfig,
    command: &str,
    cwd: Option<&str>,
    label: Option<&str>,
    timeout_seconds: Option<u64>,
) -> Result<String> {
    let mut args = json!({"command": command});
    if let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) {
        args["cwd"] = json!(cwd);
    }
    if let Some(label) = label.filter(|value| !value.trim().is_empty()) {
        args["label"] = json!(label);
    }
    if let Some(timeout_seconds) = timeout_seconds {
        args["timeout_seconds"] = json!(timeout_seconds);
    }
    start_background_task(args, config, paths, true, None)
}

/// 用户 CLI 读取后台命令输出。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `task_id`: 后台任务 ID
/// - `stream`: 输出流
/// - `tail_lines`: 读取末尾行数
///
/// 返回:
/// - JSON 格式输出
pub(crate) async fn read_background_task_output_for_user(
    paths: &SaiPaths,
    config: &AppConfig,
    task_id: &str,
    stream: &str,
    tail_lines: usize,
) -> Result<String> {
    read_background_task_output(
        json!({"task_id": task_id, "stream": stream, "tail_lines": tail_lines}),
        config,
        paths,
    )
    .await
}

/// 用户 CLI 停止后台命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `task_id`: 后台任务 ID
/// - `force`: 是否强制停止
///
/// 返回:
/// - JSON 格式停止结果
pub(crate) async fn stop_background_task_for_user(
    paths: &SaiPaths,
    config: &AppConfig,
    task_id: &str,
    force: bool,
) -> Result<String> {
    stop_background_task(json!({"task_id": task_id, "force": force}), config, paths).await
}

/// 用户 CLI 清理后台命令记录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `remove_logs`: 是否删除日志
///
/// 返回:
/// - JSON 格式清理结果
pub(crate) async fn cleanup_background_tasks_for_user(
    paths: &SaiPaths,
    config: &AppConfig,
    remove_logs: bool,
) -> Result<String> {
    cleanup_background_tasks(json!({"remove_logs": remove_logs}), paths, config).await
}
