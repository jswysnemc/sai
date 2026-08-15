use super::SubmissionSource;
use crate::agent::AgentMode;
use crate::cli::{build_repl_tool_registry_for_session, build_tool_registry};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;

/// 按 Agent 档案的 `enabled_tools` 白名单收窄注册表。
///
/// Web 与 TUI 此前各走各的构造路径，只有 Web 的 `load_tool_registry` 应用了
/// 白名单，TUI 拿到的是全量注册表，于是档案里禁用的工具照样能调、也照样写进
/// `load` 的能力清单。这里把过滤抽成公共函数，两条路径共用同一份语义。
///
/// 白名单为空表示不限制，原样返回；非空时只保留白名单内的工具，但交互类工具
/// （子智能体、计划、提问等）即使不在白名单里也要兜底加回，否则 TUI 会丧失
/// 基本交互能力。
///
/// 参数:
/// - `registry`: 过滤前的全量注册表
/// - `config`: 已应用 Agent 覆盖的运行期配置
/// - `source`: submission 来源，决定兜底保留哪些交互工具
///
/// 返回:
/// - 收窄后的注册表；非独占模式下白名单为空时与入参等价
pub(crate) fn apply_enabled_tools_filter(
    registry: ToolRegistry,
    config: &AppConfig,
    source: SubmissionSource,
) -> Result<ToolRegistry> {
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return Ok(registry);
    };
    // 非独占模式下空白名单沿用旧语义：不做收窄
    if runtime.enabled_tools.is_empty() && !runtime.exclusive {
        return Ok(registry);
    }
    // 1. 白名单收窄：只复制档案允许的工具
    let allowed = runtime
        .enabled_tools
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut filtered = registry.clone_filtered(&allowed);
    // 2. 交互工具兜底：按来源补回不在白名单里也必须保留的工具。
    //    独占白名单跳过这一步，否则"零工具"仍会带着三个交互工具
    if runtime.exclusive {
        return Ok(filtered);
    }
    let required = match source {
        SubmissionSource::Repl | SubmissionSource::Web => &["subagent", "todo", "ask_question"][..],
        SubmissionSource::Gateway => &["cron", "send_channel_message"][..],
        SubmissionSource::Command | SubmissionSource::ShellIntercept => &["ask_question"][..],
    };
    for name in required {
        if registry.contains(name) {
            filtered.register_from(&registry, name)?;
        }
    }
    Ok(filtered)
}

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
        // 单次对话同样会遇到多步任务，计划要能落到会话状态里；
        // todo 与会话绑定，同一会话的后续命令能接着读到上一轮的计划
        tools::register_todo(&mut registry, state_dir);
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
