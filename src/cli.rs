use crate::agent::AgentMode;
use crate::clipboard;
use crate::config::AppConfig;
use crate::gateways::cli::{run_gateway, GatewayArgs, GatewayCommand};
use crate::i18n::{is_zh, text as t};
use crate::llm::OpenAiCompatibleClient;
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::render;
use crate::shell;
use crate::state::StateStore;
use crate::tools;
use anyhow::{bail, Result};
use crossterm::cursor::{self, Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::io::Cursor;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod agent_select;
mod alarm_worker;
mod args;
mod background_commands;
mod center_panel;
mod chat;
mod compaction;
mod composer_tips;
mod config_commands;
mod confirm;
mod fuzzy_select;
mod history;
mod init;
mod input_flags;
mod interaction;
mod kb_commands;
mod keyboard_enhancement;
mod localization;
mod memory_commands;
mod message;
mod model_select;
mod models_picker;
mod permission_prompt;
mod providers;
mod render_options;
mod repl;
mod repl_background;
mod repl_chrome;
mod repl_clipboard;
mod repl_commands;
mod repl_editor;
mod repl_editor_buffer;
mod repl_external_events;
mod repl_input;
mod repl_input_navigation;
mod repl_input_render;
#[cfg(test)]
mod repl_input_tests;
mod repl_pager;
mod repl_runtime;
mod repl_shell;
mod repl_text;
mod repl_tool_warmup;
mod repl_transcript_pager;
mod repl_turn;
mod repl_turn_failure;
mod repl_windows_paste;
mod reset;
mod sessions;
mod skills_commands;
mod terminal_restore;
mod tree_select;

use alarm_worker::run_alarm_worker;
pub(crate) use args::*;
use background_commands::run_background_commands;
use chat::{
    run_chat_with_options, run_shell_intercept, run_stored_shell_explanation, ChatRunOptions,
};
use compaction::run_compaction;
use config_commands::run_config;
use fuzzy_select::inline_fuzzy_select;
use history::run_history;
use init::{remove_shell_hooks, run_init, InitKind};
use input_flags::parse_message_input_flags;
use interaction::{
    handle_agent_event, prompt_permission_request, prompt_permission_request_tui,
    prompt_question_request_tui,
};
use kb_commands::run_kb;
pub(crate) use localization::parse;
use memory_commands::{clear_memory, run_memory};
use message::{join_message, prepare_clipboard_chat_input};
use providers::{apply_thinking_override, run_providers, run_set, run_set_thinking};
use render_options::stream_render_options;
use repl::run_repl;
use repl_background::run_repl_background_manager;
#[cfg(test)]
use repl_commands::repl_command_suggestions;
use repl_commands::{
    complete_repl_command, repl_command_rest, unknown_slash_command_hint,
    visible_repl_command_suggestions,
};
use repl_editor::edit_input_buffer;
use repl_input::read_repl_input;
use repl_input_navigation::{move_cursor_down_by_visual_row, move_cursor_up_by_visual_row};
use repl_input_render::{clear_repl_input, render_repl_input};
use repl_runtime::{process_stream_input, process_stream_tick, ReplRuntime};
use repl_shell::execute_repl_shell;
use repl_text::*;
use reset::run_reset;
use sessions::run_sessions;
use skills_commands::run_skills;

const REPL_MAX_VISIBLE_INPUT_ROWS: u16 = 12;
const REPL_ESC_CLEAR_WINDOW: Duration = Duration::from_millis(650);
const REPL_CTRL_C_EXIT_WINDOW: Duration = Duration::from_millis(900);
const THINKING_LEVELS: &[&str] = &["auto", "none", "low", "medium", "high", "xhigh", "max"];

/// 已向用户完整展示过的失败。
///
/// 主入口捕获后只需以对应退出码结束进程，不再重复打印错误链。
#[derive(Debug)]
pub struct SilentExit {
    pub code: i32,
}

impl std::fmt::Display for SilentExit {
    /// 格式化为简短占位文本，仅在被意外打印时出现。
    ///
    /// 参数:
    /// - `f`: 格式化器
    ///
    /// 返回:
    /// - 格式化结果
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit code {}", self.code)
    }
}

impl std::error::Error for SilentExit {}

pub async fn run(cli: Cli) -> Result<()> {
    // raw mode 或备用屏内 panic 时先恢复终端，否则用户 shell 不可用
    terminal_restore::install_panic_hook();
    let paths = SaiPaths::new()?;
    let thinking_override = cli.thinking.clone();
    let mode_override = cli_mode_override(&cli);

    if cli.shell_intercept {
        let shell_name = cli.shell.as_deref().unwrap_or("fish");
        let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
        return run_shell_intercept(
            &paths,
            shell_name,
            input.message,
            input.clipb,
            input.web_search,
        )
        .await;
    }

    if !paths.config_file.exists() && !matches!(cli.command, Some(Command::Init)) {
        run_init(&paths, InitKind::FirstRun)?;
    }

    if cli.explain {
        if cli.command.is_some() {
            bail!("-e/--explain cannot be combined with a subcommand");
        }
        let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
        let mode = resolve_agent_mode(&paths, mode_override, PermissionSurface::Cli)?;
        return run_stored_shell_explanation(
            &paths,
            input.message,
            input.clipb,
            input.web_search,
            mode,
            thinking_override,
        )
        .await;
    }

    match cli.command {
        Some(Command::AlarmWorker(args)) => run_alarm_worker(args),
        Some(Command::Tool(args)) => {
            run_tool(
                &paths,
                resolve_agent_mode(&paths, mode_override, PermissionSurface::Cli)?,
                args,
            )
            .await
        }
        Some(Command::Web(args)) => crate::web::run(&paths, args).await,
        Some(Command::Ask(args)) => {
            let mode = resolve_agent_mode(&paths, mode_override, PermissionSurface::Cli)?;
            let input = parse_message_input_flags(args.message, args.clipb, args.web_search);
            run_chat_with_options(
                &paths,
                ChatRunOptions {
                    message: input.message,
                    source: crate::runner::SubmissionSource::Command,
                    show_reasoning: None,
                    plain: false,
                    mode,
                    clipb: input.clipb,
                    web_search: input.web_search,
                    thinking_override: args.thinking.or_else(|| thinking_override.clone()),
                    show_final_summary: true,
                },
            )
            .await
        }
        Some(Command::Init) => run_init(&paths, InitKind::Explicit),
        Some(Command::Paths) => {
            paths.print();
            Ok(())
        }
        Some(Command::Config(args)) => run_config(&paths, args).await,
        Some(Command::Models) => models_picker::run(&paths),
        Some(Command::Providers(args)) => run_providers(&paths, args),
        Some(Command::FishInit) => shell::fish::install(&paths),
        Some(Command::BashInit) => shell::bash::install(&paths),
        Some(Command::ZshInit) => shell::zsh::install(&paths),
        Some(Command::PowershellInit) => shell::powershell::install(&paths),
        Some(Command::RemoveShellHook) => remove_shell_hooks(&paths),
        Some(Command::History(args)) => run_history(&paths, args),
        Some(Command::Sessions(args)) => run_sessions(&paths, args),
        Some(Command::Resume(args)) => sessions::run_resume(&paths, args),
        Some(Command::Kb(args)) => run_kb(&paths, args).await,
        Some(Command::Memory(args)) => run_memory(&paths, args),
        Some(Command::Skills(args)) => run_skills(&paths, args),
        Some(Command::Ps(args)) => run_background_commands(&paths, args).await,
        Some(Command::Gateway(args)) => run_gateway(&paths, args).await,
        Some(Command::WeixinLogin(args)) => {
            run_gateway(
                &paths,
                GatewayArgs {
                    verbose: args.verbose,
                    command: GatewayCommand::WeixinLogin(args.login),
                },
            )
            .await
        }
        Some(Command::Set(args)) => run_set(&paths, args),
        Some(Command::Clear(args)) => {
            run_reset(&paths, args.scope.as_deref(), args.memory, args.yes)
        }
        Some(Command::Compact(_)) => run_compaction(&paths).await,
        None => {
            let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
            // 管道输入走一次性聊天路径，内容在 chat 层读作消息
            let piped_stdin = !io::stdin().is_terminal();
            if input.message.is_empty() && !input.clipb && !input.web_search && !piped_stdin {
                chat::ensure_interactive_terminal_for_repl()?;
                let mode = resolve_agent_mode(&paths, mode_override, PermissionSurface::Tui)?;
                run_repl(&paths, mode, thinking_override.clone()).await
            } else {
                let mode = resolve_agent_mode(&paths, mode_override, PermissionSurface::Cli)?;
                run_chat_with_options(
                    &paths,
                    ChatRunOptions {
                        message: input.message,
                        source: crate::runner::SubmissionSource::Command,
                        show_reasoning: None,
                        plain: false,
                        mode,
                        clipb: input.clipb,
                        web_search: input.web_search,
                        thinking_override: thinking_override.clone(),
                        show_final_summary: true,
                    },
                )
                .await
            }
        }
    }
}

/// 读取命令行显式指定的权限模式。
///
/// 参数:
/// - `cli`: 已解析的命令行参数
///
/// 返回:
/// - 显式模式；未指定时返回空
fn cli_mode_override(cli: &Cli) -> Option<AgentMode> {
    if cli.plan {
        Some(AgentMode::Plan)
    } else if cli.audited {
        Some(AgentMode::Audited)
    } else if cli.auto_audit {
        Some(AgentMode::AutoAudit)
    } else if cli.yolo {
        Some(AgentMode::Yolo)
    } else {
        None
    }
}

/// 合并命令行覆盖与持久化默认权限模式。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `mode_override`: 命令行显式模式
/// - `surface`: 调用入口（TUI 或 CLI）
///
/// 返回:
/// - 当前入口应采用的 Agent 模式
fn resolve_agent_mode(
    paths: &SaiPaths,
    mode_override: Option<AgentMode>,
    surface: PermissionSurface,
) -> Result<AgentMode> {
    if let Some(mode) = mode_override {
        return Ok(mode);
    }
    let config = AppConfig::load_or_default(paths)?;
    Ok(match surface {
        PermissionSurface::Tui => config.permission.tui_mode().into(),
        PermissionSurface::Cli => config.permission.cli_mode().into(),
    })
}

/// 权限默认值适用的终端入口。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PermissionSurface {
    /// 交互式 TUI REPL。
    Tui,
    /// 单次 ask/tool 等 CLI 命令。
    Cli,
}

async fn run_tool(paths: &SaiPaths, mode: AgentMode, args: ToolArgs) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    // 单工具 CLI 只需要本地工具定义，避免同步发现 MCP 服务阻塞命令执行
    let mut registry = build_tool_registry_without_mcp(&config, paths, mode)?;
    let profile_mode = mode.permission_profile_mode();
    let audit = (mode != AgentMode::Yolo).then(|| {
        crate::permission::PermissionAuditLog::new(
            paths.data_dir.join("permission-audit-cli.jsonl"),
            "cli-tool",
        )
    });
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        profile_mode,
        crate::runtime_cwd::current_dir()?,
        audit,
    ));
    let arguments = args.arguments.as_deref().unwrap_or("{}");
    if registry.requires_permission(&args.name, arguments)? {
        // 1. 先绘制既有工具视图，再在其下方补充权限选择
        println!(
            "{}",
            crate::render::render_tool_call(
                &args.name,
                arguments,
                crate::render::ToolCallDisplayMode::Full,
            )
        );
        registry.record_permission_requested(&args.name, arguments)?;
        let (request, receiver) =
            crate::permission::request_permission("cli-tool", &args.name, arguments);
        prompt_permission_request(&request)?;
        // 2. 只有批准后才进入工具注册表执行路径
        let decision = receiver.await?;
        let approval_detail = decision.detail().map(str::to_string);
        match decision {
            crate::permission::PermissionDecision::Allow { .. } => {
                registry.record_permission_approved(
                    &args.name,
                    arguments,
                    approval_detail.as_deref(),
                )?;
            }
            crate::permission::PermissionDecision::Deny { reply } => {
                registry.record_permission_denied(&args.name, arguments, reply.as_deref())?;
                let message = reply
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "用户拒绝了此工具调用".to_string());
                bail!(message)
            }
        }
    }
    let output = registry.call(&args.name, arguments).await?;
    println!("{output}");
    Ok(())
}

/// 构建通用工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式
///
/// 返回:
/// - 工具注册表
pub(crate) fn build_tool_registry(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
) -> Result<tools::ToolRegistry> {
    build_tool_registry_with_mcp(config, paths, mode, true)
}

/// 构建不连接 MCP 服务的本地工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式
///
/// 返回:
/// - 本地工具注册表
pub(crate) fn build_tool_registry_without_mcp(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
) -> Result<tools::ToolRegistry> {
    build_tool_registry_with_mcp(config, paths, mode, false)
}

/// 构建从缓存注册并在首次调用时连接 MCP 的工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式
///
/// 返回:
/// - 本地工具与延迟 MCP 工具组成的注册表
pub(crate) fn build_tool_registry_with_cached_mcp(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
) -> Result<tools::ToolRegistry> {
    let mut registry = if config.tools.enabled {
        match mode {
            AgentMode::Yolo | AgentMode::Audited | AgentMode::AutoAudit => {
                tools::builtin_registry_with_cached_mcp(config, paths)
            }
            AgentMode::Plan => tools::readonly_registry(config, paths),
        }
    } else {
        tools::ToolRegistry::new()
    };
    if mode != AgentMode::Plan && config.tools.enabled && config.skills.enabled {
        tools::register_skills(&mut registry, config, paths, true)?;
    }
    Ok(registry)
}

/// 按需构建本地或完整工具注册表。
fn build_tool_registry_with_mcp(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
    discover_mcp: bool,
) -> Result<tools::ToolRegistry> {
    let mut registry = if config.tools.enabled {
        match mode {
            AgentMode::Yolo | AgentMode::Audited | AgentMode::AutoAudit if discover_mcp => {
                tools::builtin_registry(config, paths)
            }
            AgentMode::Yolo | AgentMode::Audited | AgentMode::AutoAudit => {
                tools::builtin_registry_without_mcp(config, paths)
            }
            AgentMode::Plan => tools::readonly_registry(config, paths),
        }
    } else {
        tools::ToolRegistry::new()
    };
    if mode != AgentMode::Plan && config.tools.enabled && config.skills.enabled {
        tools::register_skills(&mut registry, config, paths, true)?;
    }
    Ok(registry)
}

pub(crate) fn build_repl_tool_registry(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
) -> Result<tools::ToolRegistry> {
    let state = crate::state::StateStore::new(paths)?;
    build_repl_tool_registry_for_session(config, paths, mode, state.session_id(), state.state_dir())
}

/// 构造绑定到指定会话的交互式工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: Agent 模式
/// - `session_id`: 会话 ID
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 工具注册表
pub(crate) fn build_repl_tool_registry_for_session(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
    session_id: &str,
    state_dir: &std::path::Path,
) -> Result<tools::ToolRegistry> {
    build_repl_tool_registry_for_session_with_mcp(config, paths, mode, session_id, state_dir, true)
}

/// 构造不连接 MCP 服务的会话工具注册表。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: Agent 模式
/// - `session_id`: 会话 ID
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 可立即用于 TUI 首屏的本地工具注册表
pub(crate) fn build_repl_tool_registry_without_mcp_for_session(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
    session_id: &str,
    state_dir: &std::path::Path,
) -> Result<tools::ToolRegistry> {
    build_repl_tool_registry_for_session_with_mcp(config, paths, mode, session_id, state_dir, false)
}

/// 按需构造本地或完整的会话工具注册表。
fn build_repl_tool_registry_for_session_with_mcp(
    config: &AppConfig,
    paths: &SaiPaths,
    mode: AgentMode,
    session_id: &str,
    state_dir: &std::path::Path,
    discover_mcp: bool,
) -> Result<tools::ToolRegistry> {
    let mut registry = if discover_mcp {
        build_tool_registry(config, paths, mode)?
    } else {
        build_tool_registry_without_mcp(config, paths, mode)?
    };
    if mode != AgentMode::Plan && config.tools.enabled {
        tools::register_interactive_tools(
            &mut registry,
            config,
            paths,
            state_dir.display().to_string(),
            session_id.to_string(),
        );
    }
    // TUI 复用 Agent 的热路径，不会经过 SessionRunner::load_tool_registry，
    // 因此白名单必须在这里应用，否则档案禁用的工具仍会全量暴露给模型
    registry = crate::runner::apply_enabled_tools_filter(
        registry,
        config,
        crate::runner::SubmissionSource::Repl,
    )?;
    let workspace = crate::runtime_cwd::current_dir()?;
    let audit = (mode != AgentMode::Yolo).then(|| {
        crate::permission::PermissionAuditLog::new(
            state_dir.join("permission-audit.jsonl"),
            session_id.to_string(),
        )
    });
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        mode.permission_profile_mode(),
        workspace,
        audit,
    ));
    Ok(registry)
}
