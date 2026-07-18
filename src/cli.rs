use crate::agent::{AgentEvent, AgentMode};
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
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
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
mod chat;
mod compaction;
mod config_commands;
mod fuzzy_select;
mod history;
mod init;
mod input_flags;
mod kb_commands;
mod localization;
mod memory_commands;
mod message;
mod model_select;
mod permission_prompt;
mod providers;
mod render_options;
mod repl;
mod repl_background;
mod repl_chrome;
mod repl_clipboard;
mod repl_commands;
mod repl_editor;
mod repl_input;
mod repl_input_navigation;
mod repl_input_render;
#[cfg(test)]
mod repl_input_tests;
mod repl_runtime;
mod repl_shell;
mod repl_text;
mod reset;
mod sessions;
mod skills_commands;

use alarm_worker::run_alarm_worker;
pub(crate) use args::*;
use background_commands::run_background_commands;
use chat::{run_chat_with_options, run_shell_intercept, ChatRunOptions};
use compaction::run_compaction;
use config_commands::run_config;
use fuzzy_select::inline_fuzzy_select;
use history::run_history;
use init::{remove_shell_hooks, run_init, InitKind};
use input_flags::parse_message_input_flags;
use kb_commands::run_kb;
pub(crate) use localization::parse;
use memory_commands::{clear_memory, run_memory};
use message::{join_message, prepare_clipboard_chat_input};
use providers::{apply_thinking_override, run_providers, run_set, run_set_thinking};
use render_options::stream_render_options;
use repl::run_repl;
use repl_background::run_repl_background_manager;
use repl_clipboard::ReplClipboardState;
use repl_commands::{complete_repl_command, repl_command_rest, repl_command_suggestions};
use repl_editor::edit_input_buffer;
use repl_input::read_repl_input;
use repl_input_navigation::{move_cursor_down_by_visual_row, move_cursor_up_by_visual_row};
use repl_input_render::{clear_repl_input, render_repl_input};
use repl_runtime::{process_stream_tick, ReplRuntime};
use repl_shell::execute_repl_shell;
use repl_text::*;
use reset::run_reset;
use sessions::run_sessions;
use skills_commands::run_skills;

const REPL_MAX_VISIBLE_INPUT_ROWS: u16 = 12;
const REPL_ESC_CLEAR_WINDOW: Duration = Duration::from_millis(650);
const REPL_CTRL_C_EXIT_WINDOW: Duration = Duration::from_millis(900);
const THINKING_LEVELS: &[&str] = &["auto", "none", "low", "medium", "high", "xhigh", "max"];

pub async fn run(cli: Cli) -> Result<()> {
    let paths = SaiPaths::new()?;
    let thinking_override = cli.thinking.clone();
    let mode_override = cli_mode_override(&cli);

    if cli.shell_intercept {
        let shell_name = cli.shell.as_deref().unwrap_or("fish");
        let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
        let mode = resolve_agent_mode(&paths, mode_override, PermissionSurface::Cli)?;
        return run_shell_intercept(
            &paths,
            shell_name,
            input.message,
            input.clipb,
            input.web_search,
            mode,
        )
        .await;
    }

    if !paths.config_file.exists() && !matches!(cli.command, Some(Command::Init)) {
        run_init(&paths, InitKind::FirstRun)?;
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
        Some(Command::Clear(args)) => run_reset(&paths, args.scope.as_deref(), args.memory),
        Some(Command::Compact(_)) => run_compaction(&paths).await,
        None => {
            let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
            if input.message.is_empty() && !input.clipb && !input.web_search {
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
    let mut registry = build_tool_registry(&config, paths, mode)?;
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
        match receiver.await? {
            crate::permission::PermissionDecision::Allow => {
                registry.record_permission_approved(&args.name, arguments)?;
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
    let mut registry = if config.tools.enabled {
        match mode {
            AgentMode::Yolo => tools::builtin_registry(config, paths),
            AgentMode::Audited => tools::builtin_registry(config, paths),
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
    let mut registry = build_tool_registry(config, paths, mode)?;
    if mode != AgentMode::Plan && config.tools.enabled {
        tools::register_interactive_tools(
            &mut registry,
            config,
            paths,
            state_dir.display().to_string(),
            session_id.to_string(),
        );
    }
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

/// 将单次 CLI Agent 事件转发到流式渲染器或权限交互。
///
/// 参数:
/// - `renderer`: CLI 流式渲染器
/// - `event`: Agent 事件
///
/// 返回:
/// - 事件处理结果
fn handle_agent_event(renderer: &mut render::StreamRenderer, event: AgentEvent) -> Result<()> {
    match event {
        AgentEvent::Chunk(chunk) => renderer.write_chunk(chunk),
        AgentEvent::ToolCall { name, arguments } => renderer.write_tool_call(&name, &arguments),
        AgentEvent::ToolCallProgress(progress) => renderer.write_tool_call_progress(&progress),
        AgentEvent::ToolResult { name, ok, output } => {
            renderer.write_tool_result(&name, ok, &output)
        }
        AgentEvent::ToolProgress { name, message } => renderer.write_tool_progress(&name, &message),
        AgentEvent::PermissionRequested(request) => {
            // 停掉末行动效与 live 行，再在 stdout 画可导航审计菜单
            renderer.prepare_for_external_output()?;
            io::stdout().flush()?;
            let decision = prompt_permission_request(&request)?;
            // 拒绝决定已单独展示，抑制随后同名工具的失败输出块避免重复
            if matches!(
                decision,
                crate::permission::PermissionDecision::Deny { .. }
            ) {
                renderer.suppress_denied_result(&request.tool);
            }
            Ok(())
        }
        AgentEvent::PermissionResolved { .. } => Ok(()),
        AgentEvent::QuestionRequested(pending) => {
            renderer.prepare_for_external_output()?;
            io::stdout().flush()?;
            prompt_question_request(&pending)
        }
        AgentEvent::QuestionResolved { .. } => Ok(()),
        AgentEvent::CompactionStarted { turn_count, model } => {
            renderer.write_compaction_started(turn_count, &model)
        }
        AgentEvent::CompactionDelta { text } => renderer.write_compaction_delta(text),
        AgentEvent::CompactionFinished { applied, error, .. } => {
            renderer.write_compaction_finished(applied, error.as_ref())
        }
        AgentEvent::FlushContent => renderer.flush_content(),
        AgentEvent::ExternalOutput => renderer.prepare_for_external_output(),
    }
}

/// 在终端读取权限允许、拒绝或拒绝原因。
///
/// 参数:
/// - `request`: 待处理权限请求
///
/// 返回:
/// - 已提交给权限 Broker 的用户决定
fn prompt_permission_request(
    request: &crate::permission::PermissionRequest,
) -> Result<crate::permission::PermissionDecision> {
    // 1. 先把工具输出刷到屏幕，再画权限菜单（写到 stdout，避免被 stderr 错位）
    let decision = permission_prompt::read_permission_decision(request)?;
    crate::permission::decide_permission(&request.id, decision.clone())?;
    println!("{}", crate::render::render_permission_decision(&decision));
    Ok(decision)
}

/// 在 TUI 原始模式中读取权限选择，并更新既有工具视图。
///
/// 参数:
/// - `request`: 已经写入 transcript 的权限请求
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 权限决定提交结果
fn prompt_permission_request_tui(
    request: &crate::permission::PermissionRequest,
    runtime: &std::cell::RefCell<&mut ReplRuntime>,
) -> Result<()> {
    use crate::permission::{PermissionInteractionState, PermissionTransition};
    use repl_input::{disable_repl_terminal_input, enable_repl_terminal_input};

    let mut state = PermissionInteractionState::new();
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    enable_repl_terminal_input(&mut stdout)?;
    // 2. 暂停工作动效，选择项附着在工具视图下方
    {
        let mut rt = runtime.borrow_mut();
        rt.pause_for_permission_prompt()?;
        rt.update_permission_choice(&request.id, state.selected())?;
        rt.update_permission_reply(&request.id, state.reply_draft().map(str::to_string))?;
    }

    let result = (|| -> Result<()> {
        loop {
            let event = event::read()?;
            // Ctrl+C / Ctrl+D 视为拒绝，避免审计循环无法退出
            if permission_prompt::is_interrupt(&event) {
                return crate::permission::decide_permission(
                    &request.id,
                    crate::permission::PermissionDecision::Deny { reply: None },
                );
            }
            if let Event::Resize(cols, rows) = event {
                let mut rt = runtime.borrow_mut();
                rt.observe_input_resize(cols, rows);
                rt.update_permission_choice(&request.id, state.selected())?;
                rt.update_permission_reply(&request.id, state.reply_draft().map(str::to_string))?;
                continue;
            }
            match state.handle_event(event) {
                PermissionTransition::Continue => {
                    let mut rt = runtime.borrow_mut();
                    rt.update_permission_choice(&request.id, state.selected())?;
                    rt.update_permission_reply(
                        &request.id,
                        state.reply_draft().map(str::to_string),
                    )?;
                }
                PermissionTransition::Submit(decision) => {
                    return crate::permission::decide_permission(&request.id, decision);
                }
            }
        }
    })();

    // 3. 恢复终端模式，交回后续流式输出和下一轮输入
    let _ = disable_repl_terminal_input(&mut stdout);
    result
}

/// 在终端读取结构化提问答案。
///
/// 参数:
/// - `pending`: 待回答提问
///
/// 返回:
/// - 是否成功提交回答
fn prompt_question_request(pending: &crate::question::PendingQuestion) -> Result<()> {
    let response = crate::question_tui::ask(&pending.request)
        .unwrap_or_else(|err| crate::question::QuestionResponse::Unavailable(err.to_string()));
    crate::question::resolve_question(&pending.id, response)
}

/// 在 TUI 原始模式中读取结构化提问答案。
///
/// 参数:
/// - `pending`: 待回答提问
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 是否成功提交回答
fn prompt_question_request_tui(
    pending: &crate::question::PendingQuestion,
    runtime: &std::cell::RefCell<&mut ReplRuntime>,
) -> Result<()> {
    use repl_input::{disable_repl_terminal_input, enable_repl_terminal_input};

    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    enable_repl_terminal_input(&mut stdout)?;
    {
        let mut rt = runtime.borrow_mut();
        rt.pause_for_permission_prompt()?;
    }

    let response = crate::question_tui::ask(&pending.request)
        .unwrap_or_else(|err| crate::question::QuestionResponse::Unavailable(err.to_string()));

    // 2. 恢复终端模式；提问面板直接写过终端，受管区域需要在下次同步前重启
    let _ = disable_repl_terminal_input(&mut stdout);
    runtime.borrow_mut().mark_desynced();
    crate::question::resolve_question(&pending.id, response)
}
