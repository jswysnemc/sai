use super::repl_chrome::ReplChrome;
use super::repl_external_events::ReplExternalEvents;
use super::repl_input::ReplInputEvent;
use super::repl_tool_warmup::ReplToolWarmup;
use super::repl_turn::{execute_automatic_repl_turn, execute_repl_turn};
use super::*;
use crate::agent::Agent;

const REPL_HISTORY_TURN_LIMIT: usize = 50;

pub(super) async fn run_repl(
    paths: &SaiPaths,
    initial_mode: AgentMode,
    thinking_override: Option<String>,
) -> Result<()> {
    AppConfig::init_files(paths)?;
    let mut config = crate::config::apply_agent_override(
        AppConfig::load_or_default(paths)?,
        None,
        crate::config::AgentSurface::Tui,
    )?;
    apply_thinking_override(&mut config, thinking_override.as_deref())?;
    let mut state = StateStore::new(paths)?;
    state.init_files()?;
    let mut client = OpenAiCompatibleClient::from_config(&config, paths)?;
    let mut mode = initial_mode;
    let mut input_history = load_repl_input_history(&state)?;
    let mut prefill = None::<String>;
    let mut prefill_clipboard = None;
    let initial_transcript_options = render::transcript::TranscriptRenderOptions {
        reasoning_mode: render::ReasoningDisplayMode::from_config(&config.display.reasoning),
        tool_call_mode: render::ToolCallDisplayMode::from_config(&config.display.tool_calls),
    };
    // 光标不在行首时先换行，避免受管区域首行覆盖 shell 残留输出
    if crossterm::cursor::position()
        .map(|(col, _)| col != 0)
        .unwrap_or(false)
    {
        println!();
    }
    let mut runtime = ReplRuntime::new(
        config.display.repl_transcript_row_cap,
        initial_transcript_options,
    );
    runtime.record_welcome(
        env!("CARGO_PKG_VERSION").to_string(),
        repl_welcome_model(&config),
        crate::runtime_cwd::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "~".to_string()),
        format!("{} mode", mode.label()),
    )?;
    runtime.record_meta(
        t(
            "Tab mode · Enter send · Shift+Enter newline · Ctrl+V paste",
            "Tab 模式 · Enter 发送 · Shift+Enter 换行 · Ctrl+V 粘贴",
        )
        .to_string(),
    )?;
    record_repl_history(&mut runtime, &state)?;
    // 1. 重量级初始化前先呈现输入框，避免版本信息后长时间没有输入区
    {
        let chrome = ReplChrome::from_runtime(&config, &state, mode);
        runtime.update_composer(&chrome, "", 0, false, Vec::new(), 0)?;
        runtime.draw_composer(&mut std::io::stdout())?;
    }
    // 2. 本地工具立即可用，MCP 动态工具在后台发现，避免阻塞输入框
    let initial_registry = build_repl_tool_registry_without_mcp_for_session(
        &config,
        paths,
        mode,
        state.session_id(),
        state.state_dir(),
    )?;
    let mut tool_warmup = ReplToolWarmup::start(
        config.clone(),
        paths.clone(),
        mode,
        state.session_id().to_string(),
        state.state_dir().to_path_buf(),
    );
    let mut agent = Agent::new(
        config.clone(),
        paths,
        state.clone(),
        client.clone(),
        initial_registry,
        mode,
    )?;
    let mut external_events = ReplExternalEvents::new();

    loop {
        // 每次进入输入循环都重新绑定当前 Agent，会话切换后不会消费旧监听结果
        external_events.arm(&agent);
        apply_ready_tool_registry(&mut tool_warmup, &mut agent, mode, &mut runtime)?;
        // 每轮刷新底栏上下文/模型信息
        let mut chrome = ReplChrome::from_runtime(&config, &state, mode);
        let transcript_options = render::transcript::TranscriptRenderOptions {
            reasoning_mode: render::ReasoningDisplayMode::from_config(&config.display.reasoning),
            tool_call_mode: render::ToolCallDisplayMode::from_config(&config.display.tool_calls),
        };
        runtime.update_options(config.display.repl_transcript_row_cap, transcript_options);
        let submission = match read_repl_input(
            mode,
            prefill.take(),
            prefill_clipboard.take(),
            &input_history,
            &mut chrome,
            &mut runtime,
            &mut external_events,
        )? {
            Some(ReplInputEvent::User(submission)) => {
                mode = submission.mode;
                submission
            }
            Some(ReplInputEvent::Automatic {
                mode: automatic_mode,
                wake,
                draft,
            }) => {
                mode = automatic_mode;
                prefill = (!draft.text.trim().is_empty()).then_some(draft.text);
                prefill_clipboard = prefill.as_ref().map(|_| draft.clipboard_state);
                apply_ready_tool_registry(&mut tool_warmup, &mut agent, mode, &mut runtime)?;
                let outcome = execute_automatic_repl_turn(
                    paths,
                    &config,
                    &mut agent,
                    &mut runtime,
                    mode,
                    transcript_options.reasoning_mode,
                    transcript_options.tool_call_mode,
                    wake,
                )
                .await?;
                if outcome.interrupted {
                    if let Some(error) = outcome.result.err() {
                        runtime.record_meta(error.to_string())?;
                    }
                } else if let Err(error) = outcome.result {
                    runtime.record_meta(error.to_string())?;
                }
                continue;
            }
            None => break,
        };
        apply_ready_tool_registry(&mut tool_warmup, &mut agent, mode, &mut runtime)?;
        let input = submission.raw_input.trim();
        let mut submitted_input = input.to_string();
        if input.eq_ignore_ascii_case("exit")
            || input.eq_ignore_ascii_case("quit")
            || input.eq_ignore_ascii_case("/exit")
        {
            break;
        }
        if let Some(command) = input.strip_prefix('!') {
            match execute_repl_shell(command).await {
                Ok(result) => {
                    runtime.record_shell(result.command, result.output, result.exit_code)?
                }
                Err(err) => runtime.record_meta(err.to_string())?,
            }
            continue;
        }
        let mut goal_continuation = false;
        match crate::control_commands::parse_control_command(
            input,
            crate::control_commands::ControlSurface::Repl,
        ) {
            Ok(Some(command)) => {
                match command {
                    crate::control_commands::ControlCommand::Help => {
                        runtime.record_meta(crate::control_commands::help_text(
                            crate::control_commands::ControlSurface::Repl,
                        ))?;
                    }
                    crate::control_commands::ControlCommand::New { title } => {
                        let message = crate::control_commands::create_new_session(paths, &title)?;
                        state = StateStore::new(paths)?;
                        state.init_files()?;
                        agent.replace_state(state.clone())?;
                        input_history = load_repl_input_history(&state)?;
                        prefill = None;
                        runtime.clear()?;
                        runtime.record_meta(message)?;
                    }
                    crate::control_commands::ControlCommand::Resume { id } => {
                        let session_id = match id {
                            Some(id) => id,
                            None => match sessions::select_session_id_interactively(paths) {
                                Ok(id) => id,
                                Err(err) => {
                                    runtime.record_meta(err.to_string())?;
                                    continue;
                                }
                            },
                        };
                        match crate::control_commands::resume_session(paths, &session_id) {
                            Ok(message) => {
                                state = StateStore::new(paths)?;
                                state.init_files()?;
                                agent.replace_state(state.clone())?;
                                input_history = load_repl_input_history(&state)?;
                                prefill = None;
                                runtime.clear()?;
                                runtime.record_meta(message)?;
                                record_repl_history(&mut runtime, &state)?;
                            }
                            Err(err) => runtime.record_meta(err.to_string())?,
                        }
                    }
                    crate::control_commands::ControlCommand::Compact => {
                        let submission = crate::runner::RunnerSubmission::control(
                            crate::runner::SubmissionSource::Repl,
                            mode,
                            crate::runner::ControlSubmission::new(
                                crate::control_commands::ControlCommand::Compact,
                            ),
                        );
                        let result = {
                            let runner = crate::runner::SessionRunner::new(paths)
                                .with_config(config.clone());
                            let runtime = std::cell::RefCell::new(&mut runtime);
                            let mut sink = |event: crate::runner::RunnerEvent| {
                                runtime.borrow_mut().record_runner_event(&event)
                            };
                            let compact =
                                runner.run_submission_with_agent(submission, &mut agent, &mut sink);
                            tokio::pin!(compact);
                            let mut resize_tick = tokio::time::interval(Duration::from_millis(25));
                            resize_tick
                                .set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                            let ctrl_c = tokio::signal::ctrl_c();
                            tokio::pin!(ctrl_c);
                            loop {
                                tokio::select! {
                                    result = &mut compact => break result.map(|_| ()),
                                    signal = &mut ctrl_c => {
                                        signal?;
                                        break Ok(());
                                    }
                                    _ = resize_tick.tick() => {
                                        let mut runtime_ref = runtime.borrow_mut();
                                        process_stream_tick(&mut *runtime_ref)?;
                                    }
                                }
                            }
                        };
                        runtime.finish_stream()?;
                        result?;
                        input_history = load_repl_input_history(&state)?;
                    }
                    crate::control_commands::ControlCommand::Clear { all } => {
                        let message = crate::control_commands::clear_state(paths, all)?;
                        input_history.clear();
                        // 3. 会话清空后刷新 Agent 状态；all 时重建记忆
                        state = StateStore::new(paths)?;
                        state.init_files()?;
                        agent.replace_state(state.clone())?;
                        if all {
                            agent.reset_memory()?;
                        }
                        runtime.clear()?;
                        runtime.record_meta(message)?;
                    }
                    crate::control_commands::ControlCommand::ClearMemory => {
                        let message = clear_memory(paths, false)?;
                        agent.reset_memory()?;
                        runtime.record_meta(message)?;
                    }
                    crate::control_commands::ControlCommand::Model { selection } => {
                        let selection = match selection {
                            Some(index) => Some(index),
                            None => match model_select::select_model_index_interactively(paths) {
                                Ok(index) => index,
                                Err(err) => {
                                    runtime.record_meta(err.to_string())?;
                                    continue;
                                }
                            },
                        };
                        let Some(selection) = selection else {
                            runtime.record_meta(
                                t("model selection cancelled", "已取消模型选择").to_string(),
                            )?;
                            continue;
                        };
                        match crate::control_commands::run_model_command(
                            paths,
                            Some(selection),
                            crate::control_commands::ControlSurface::Repl,
                        ) {
                            Ok(result) => {
                                runtime.record_meta(result.message)?;
                                if result.changed {
                                    reload_repl_agent(
                                        paths,
                                        &mut config,
                                        &mut client,
                                        &mut agent,
                                        mode,
                                        thinking_override.as_deref(),
                                    )?;
                                    runtime.record_meta(
                                        t("configuration reloaded", "配置已重新加载").to_string(),
                                    )?;
                                }
                            }
                            Err(err) => runtime.record_meta(err.to_string())?,
                        }
                    }
                    crate::control_commands::ControlCommand::Agent { selection } => {
                        let selection = match selection {
                            Some(index) => Some(index),
                            None => match agent_select::select_agent_index_interactively(paths) {
                                Ok(index) => index,
                                Err(err) => {
                                    runtime.record_meta(err.to_string())?;
                                    continue;
                                }
                            },
                        };
                        let Some(selection) = selection else {
                            runtime.record_meta(
                                t("agent selection cancelled", "已取消 Agent 选择").to_string(),
                            )?;
                            continue;
                        };
                        match crate::control_commands::run_agent_command(
                            paths,
                            Some(selection),
                            crate::control_commands::ControlSurface::Repl,
                        ) {
                            Ok(result) => {
                                runtime.record_meta(result.message)?;
                                if result.changed {
                                    reload_repl_agent(
                                        paths,
                                        &mut config,
                                        &mut client,
                                        &mut agent,
                                        mode,
                                        thinking_override.as_deref(),
                                    )?;
                                    runtime.record_meta(
                                        t("configuration reloaded", "配置已重新加载").to_string(),
                                    )?;
                                }
                            }
                            Err(err) => runtime.record_meta(err.to_string())?,
                        }
                    }
                    crate::control_commands::ControlCommand::Goal(command) => {
                        match crate::control_commands::execute_goal_command(&state, command) {
                            Ok(outcome) => {
                                runtime.record_meta(outcome.message)?;
                                goal_continuation = outcome.should_continue;
                                if goal_continuation {
                                    submitted_input.clear();
                                }
                            }
                            Err(error) => runtime.record_meta(error.to_string())?,
                        }
                    }
                }
                if !goal_continuation {
                    continue;
                }
            }
            Ok(None) => {}
            Err(err) => {
                runtime.record_meta(err.to_string())?;
                continue;
            }
        }
        if input.eq_ignore_ascii_case("/help") {
            runtime.record_meta(crate::control_commands::help_text(
                crate::control_commands::ControlSurface::Repl,
            ))?;
            continue;
        }
        if input.eq_ignore_ascii_case("/plan") {
            mode = AgentMode::Plan;
            runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
            continue;
        }
        if input.eq_ignore_ascii_case("/audit") {
            mode = AgentMode::Audited;
            runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
            continue;
        }
        if input.eq_ignore_ascii_case("/yolo") {
            mode = AgentMode::Yolo;
            runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
            continue;
        }
        if input.eq_ignore_ascii_case("/providers") {
            run_providers(paths, ProvidersArgs { index: None })?;
            reload_repl_agent(
                paths,
                &mut config,
                &mut client,
                &mut agent,
                mode,
                thinking_override.as_deref(),
            )?;
            runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
            continue;
        }
        if input.eq_ignore_ascii_case("/config") {
            crate::config_tui::run(paths)?;
            reload_repl_agent(
                paths,
                &mut config,
                &mut client,
                &mut agent,
                mode,
                thinking_override.as_deref(),
            )?;
            runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
            continue;
        }
        if input.eq_ignore_ascii_case("/undo") {
            let outcome = state.undo_last_turn()?;
            runtime.record_meta(format!(
                "{}: {}",
                t("undone messages", "已撤销消息数"),
                outcome.removed
            ))?;
            prefill = outcome.prompt;
            continue;
        }
        if input.eq_ignore_ascii_case("/clear") {
            run_reset(paths, None, false)?;
            input_history.clear();
            state = StateStore::new(paths)?;
            state.init_files()?;
            agent.replace_state(state.clone())?;
            runtime.clear()?;
            continue;
        }
        if input.eq_ignore_ascii_case("/clear all") {
            run_reset(paths, Some("all"), false)?;
            input_history.clear();
            state = StateStore::new(paths)?;
            state.init_files()?;
            agent.replace_state(state.clone())?;
            agent.reset_memory()?;
            runtime.clear()?;
            continue;
        }
        if let Some(rest) = repl_command_rest(input, "/thinking") {
            let level = rest
                .split_whitespace()
                .next()
                .map(std::string::ToString::to_string);
            if let Err(err) = run_set_thinking(paths, SetThinkingArgs { level }) {
                runtime.record_meta(err.to_string())?;
                continue;
            }
            reload_repl_agent(
                paths,
                &mut config,
                &mut client,
                &mut agent,
                mode,
                thinking_override.as_deref(),
            )?;
            runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
            continue;
        }
        if input.eq_ignore_ascii_case("/ps") {
            run_repl_background_manager(paths, &config).await?;
            continue;
        }
        if input.is_empty() {
            continue;
        }
        let chat_input = submission.chat_input;
        if chat_input.message.trim().is_empty() && chat_input.image_url.is_none() {
            continue;
        }
        if !goal_continuation && !input.trim().is_empty() {
            input_history.push(input.to_string());
        }
        if !goal_continuation {
            runtime.record_user(mode, input.to_string())?;
        }
        // 4. 模式变化时换工具表；每轮只做轻量 prepare
        if agent.mode() != mode {
            let registry = build_repl_tool_registry(&config, paths, mode)?;
            agent.switch_mode(mode, registry);
        }
        agent.prepare_for_turn()?;
        let reasoning_mode = transcript_options.reasoning_mode;
        let tool_call_mode = transcript_options.tool_call_mode;
        let render_options = stream_render_options(&config);
        let runner_submission = repl_runner_submission(
            chat_input,
            mode,
            reasoning_mode,
            tool_call_mode,
            render_options.clone(),
            goal_continuation,
        );
        let outcome =
            execute_repl_turn(paths, &config, &mut agent, &mut runtime, runner_submission).await?;
        if outcome.interrupted {
            if !state.latest_interrupted_turn_has_content(&submitted_input)? {
                prefill = Some(submitted_input);
            }
            continue;
        }
        if let Err(error) = outcome.result {
            runtime.record_meta(error.to_string())?;
            continue;
        }
    }
    Ok(())
}

/// 读取当前会话最近的持久化轮次并渲染到 TUI。
///
/// 参数:
/// - `runtime`: 当前 TUI 运行期
/// - `state`: 当前会话状态存储
///
/// 返回:
/// - 历史读取与渲染是否成功
fn record_repl_history(runtime: &mut ReplRuntime, state: &StateStore) -> Result<()> {
    let timeline = state.session_timeline_with_compaction(REPL_HISTORY_TURN_LIMIT)?;
    runtime.record_history_with_compaction(&timeline.turns, timeline.compaction.as_ref())
}

/// 将后台发现完成的 MCP 工具无阻塞合并到当前 Agent。
///
/// 参数:
/// - `warmup`: MCP 工具预热任务
/// - `agent`: 当前复用的 Agent
/// - `mode`: 当前输入选择的模式
/// - `runtime`: TUI 运行期，用于展示后台错误
///
/// 返回:
/// - 合并或错误展示是否成功
fn apply_ready_tool_registry(
    warmup: &mut ReplToolWarmup,
    agent: &mut Agent,
    mode: AgentMode,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let Some(result) = warmup.take_ready() else {
        return Ok(());
    };
    match result {
        Ok((warmup_mode, registry)) if warmup_mode == mode => agent.replace_tools(registry),
        Ok(_) => {}
        Err(error) => runtime.record_meta(format!(
            "{}: {error}",
            t("MCP tool discovery failed", "MCP 工具发现失败")
        ))?,
    }
    Ok(())
}

/// 构造 REPL 单轮 runner submission。
///
/// 参数:
/// - `chat_input`: 剪贴板处理后的聊天输入
/// - `mode`: 当前 Agent 模式
/// - `reasoning_mode`: 推理内容显示方式
/// - `tool_call_mode`: 工具调用显示方式
/// - `render_options`: 流式渲染选项
///
/// 返回:
/// - runner submission
fn repl_runner_submission(
    chat_input: clipboard::ClipboardChatInput,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    render_options: render::StreamRenderOptions,
    goal_continuation: bool,
) -> crate::runner::RunnerSubmission {
    let mut user_input = match chat_input.image_url {
        Some(image_url) => crate::runner::UserInputSubmission::new(chat_input.message, mode)
            .with_image_url(image_url),
        None => crate::runner::UserInputSubmission::new(chat_input.message, mode),
    };
    if goal_continuation {
        user_input = user_input.with_goal_continuation();
    }
    crate::runner::RunnerSubmission::user_input(crate::runner::SubmissionSource::Repl, user_input)
        .with_render_policy(crate::runner::RenderPolicy::new(
            false,
            reasoning_mode,
            tool_call_mode,
            render_options,
        ))
}

/// 返回欢迎面板中展示的当前模型名称。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 当前模型名称，未配置时返回占位符
fn repl_welcome_model(config: &AppConfig) -> String {
    config
        .provider(None)
        .ok()
        .map(|provider| provider.default_model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

/// 重载配置与客户端，并同步到复用中的 Agent。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 可变配置
/// - `client`: 可变 LLM 客户端
/// - `agent`: 复用中的 Agent
/// - `mode`: 当前模式
/// - `thinking_override`: 命令行思考等级覆盖
///
/// 返回:
/// - 重载是否成功
fn reload_repl_agent(
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    mode: AgentMode,
    thinking_override: Option<&str>,
) -> Result<()> {
    *config = crate::config::apply_agent_override(
        AppConfig::load(paths)?,
        None,
        crate::config::AgentSurface::Tui,
    )?;
    apply_thinking_override(config, thinking_override)?;
    *client = OpenAiCompatibleClient::from_config(config, paths)?;
    let registry = build_repl_tool_registry(config, paths, mode)?;
    agent.reload(config.clone(), client.clone(), registry, mode)?;
    Ok(())
}

pub(super) fn load_repl_input_history(state: &StateStore) -> Result<Vec<String>> {
    Ok(state
        .load_conversation()?
        .into_iter()
        .filter(|entry| {
            entry.role == "user"
                && !entry.content.trim().is_empty()
                && !crate::goal::is_continuation_input(&entry.content)
        })
        .map(|entry| strip_terminal_control_sequences(&entry.content))
        .filter(|content| !content.trim().is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 REPL 聊天输入会构造成 runner submission。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn repl_chat_input_builds_runner_submission() {
        let submission = repl_runner_submission(
            clipboard::ClipboardChatInput {
                message: "继续".to_string(),
                image_url: Some("data:image/png;base64,AAAA".to_string()),
            },
            AgentMode::Yolo,
            render::ReasoningDisplayMode::Summary,
            render::ToolCallDisplayMode::Summary,
            render::StreamRenderOptions::default(),
            false,
        );

        assert_eq!(submission.source, crate::runner::SubmissionSource::Repl);
        assert!(matches!(
            submission.kind,
            crate::runner::RunnerSubmissionKind::UserInput(crate::runner::UserInputSubmission {
                image_urls,
                ..
            }) if image_urls.len() == 1
        ));
    }

    /// 验证 REPL 审计模式构造的工具注册表绑定了权限配置。
    #[test]
    fn audited_repl_registry_intercepts_tools_before_execution() {
        let paths = SaiPaths::new().unwrap();
        let config = AppConfig::load_or_default(&paths).unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        let registry = build_repl_tool_registry_for_session(
            &config,
            &paths,
            AgentMode::Audited,
            "test-session",
            state_dir.path(),
        )
        .unwrap();

        assert!(registry
            .requires_permission("edit_file", r#"{"path":"src/main.rs","content":"x"}"#)
            .unwrap());
        let sensitive_read = if cfg!(windows) {
            r#"{"path":"C:\\Windows\\System32\\drivers\\etc\\hosts"}"#
        } else {
            r#"{"path":"/etc/hosts"}"#
        };
        assert!(registry
            .requires_permission("read_file", sensitive_read)
            .unwrap());
        assert!(!registry
            .requires_permission("read_file", r#"{"path":"src/main.rs"}"#)
            .unwrap());
        assert!(!registry
            .requires_permission("todo", r#"{"action":"add","text":"检查"}"#)
            .unwrap());
    }
}
