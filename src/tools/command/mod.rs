mod background;
mod background_actions;
mod background_runtime;
mod background_schema;
mod background_tasks;
pub(crate) mod background_timeout;
mod goal_completions;
mod process;
mod progress;
mod run;
mod store;

use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::ToolRegistry;

use background_tasks::BackgroundRuntimeOwner;

pub(crate) use background::{
    cleanup_background_tasks_for_user, list_background_tasks_for_user,
    read_background_task_output_for_user, start_background_task_for_user,
    stop_background_task_for_user,
};
pub(crate) use goal_completions::{
    acknowledge_background_completions, poll_background_completions,
    poll_session_background_completions, BackgroundCompletionNotice,
};
pub(crate) use process::{process_exists, spawn_background_shell, terminate_process};
#[cfg(test)]
pub(crate) use progress::encode_command_output as encode_command_output_for_test;
pub(crate) use progress::{decode_command_output, CommandOutputChunk, CommandOutputStream};
pub(crate) use store::{unix_seconds, BackgroundCommandStore, BackgroundCommandTask};

/// 注册命令相关工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allow_command_execution`: 是否允许执行写入类命令
pub(crate) fn register(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    allow_command_execution: bool,
) {
    run::register(registry, config, paths, allow_command_execution, None);
    if config.tools.background_commands_enabled {
        background::register(
            registry,
            config.clone(),
            paths.clone(),
            allow_command_execution,
        );
    }
}

/// 为命令模式重注册后台命令工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `session_id`: 会话标识
pub(crate) fn register_command_mode_background(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    session_id: &str,
) {
    let owner = BackgroundRuntimeOwner::command_mode(session_id);
    run::register(registry, config, paths, true, Some(owner.clone()));
    if config.tools.background_commands_enabled {
        background::register_with_runtime_owner(
            registry,
            config.clone(),
            paths.clone(),
            true,
            Some(owner),
        );
    }
}

/// 为交互式会话绑定后台命令工具 owner。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `session_id`: 会话标识
pub(crate) fn register_session_background(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    session_id: &str,
) {
    let owner = BackgroundRuntimeOwner::session(session_id);
    run::register(registry, config, paths, true, Some(owner.clone()));
    if config.tools.background_commands_enabled {
        background::register_with_runtime_owner(
            registry,
            config.clone(),
            paths.clone(),
            true,
            Some(owner),
        );
    }
}

/// 注册只读命令相关工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
pub(crate) fn register_readonly(registry: &mut ToolRegistry, config: &AppConfig, paths: &SaiPaths) {
    run::register_readonly(registry, config);
    if config.tools.background_commands_enabled {
        background::register_readonly(registry, config.clone(), paths.clone());
    }
}
