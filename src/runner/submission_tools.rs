use super::SubmissionSource;
use crate::agent::AgentMode;
use crate::cli::{build_repl_tool_registry_for_session, build_tool_registry};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;

/// 构造当前 submission 使用的工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
/// - `source`: submission 来源
/// - `mode`: Agent 模式
/// - `session_id`: 当前会话标识
/// - `state_dir`: 当前会话状态目录
///
/// 返回:
/// - 工具注册表
pub(super) fn build_submission_tool_registry(
    config: &AppConfig,
    paths: &SaiPaths,
    source: SubmissionSource,
    mode: AgentMode,
    session_id: &str,
    state_dir: &std::path::Path,
) -> Result<ToolRegistry> {
    let mut registry = match source {
        SubmissionSource::Repl | SubmissionSource::Web => {
            build_repl_tool_registry_for_session(config, paths, mode, session_id, state_dir)
        }
        source if should_discover_mcp(source) => build_tool_registry(config, paths, mode),
        _ => crate::cli::build_tool_registry_with_cached_mcp(config, paths, mode),
    }?;
    // CLI 单次对话也可提问；网关没有交互面，不注册 ask_question
    if matches!(
        source,
        SubmissionSource::Command | SubmissionSource::ShellIntercept
    ) && mode != AgentMode::Plan
        && config.tools.enabled
    {
        tools::register_ask_question(&mut registry);
    }
    if mode != AgentMode::Plan && should_apply_command_mode_exit_policy(source) {
        tools::register_command_mode_background(&mut registry, config, paths, session_id);
    }
    Ok(registry)
}

/// 判断指定提交来源是否需要同步发现 MCP 工具。
///
/// 参数:
/// - `source`: 当前提交来源
///
/// 返回:
/// - 长生命周期入口返回 `true`，短生命周期命令入口返回 `false`
pub(super) fn should_discover_mcp(source: SubmissionSource) -> bool {
    !matches!(
        source,
        SubmissionSource::Command | SubmissionSource::ShellIntercept
    )
}

/// 判断当前 submission 是否使用命令模式运行时清理策略。
///
/// 参数:
/// - `source`: submission 来源
///
/// 返回:
/// - 是否应用命令模式退出策略
pub(super) fn should_apply_command_mode_exit_policy(source: SubmissionSource) -> bool {
    matches!(
        source,
        SubmissionSource::Command | SubmissionSource::ShellIntercept
    )
}
