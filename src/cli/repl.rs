use super::repl_chrome::ReplChrome;
use super::repl_external_events::ReplExternalEvents;
use super::repl_input::ReplInputEvent;
use super::repl_tool_warmup::ReplToolWarmup;
use super::repl_turn::{execute_automatic_repl_turn, execute_repl_turn};
use super::repl_turn_failure::{interrupted_failure_text, turn_failure_text};
use super::*;
use crate::agent::Agent;

mod session_support;
mod submission_queue;

use session_support::{
    apply_ready_tool_registry, record_repl_history, reload_repl_agent, repl_welcome_model,
};
use submission_queue::{
    apply_stream_mode, drain_submission_queue, repl_runner_submission, take_stream_draft_prefill,
};

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
    // 输入历史跨会话共享：切换或新建会话都不清空，上限见 INPUT_HISTORY_LIMIT
    let mut input_history = crate::state::input_history::load_input_history(paths)?;
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
            "Shift+Tab mode · Tab queues while working · Enter send · Shift+Enter newline",
            "Shift+Tab 切模式 · 工作时 Tab 入队 · Enter 发送 · Shift+Enter 换行",
        )
        .to_string(),
    )?;
    // 使用外部内核时明确告知：对话交给了谁、哪些 sai 功能因此停用，
    // 否则压缩与记忆静默失效，用起来像是出了故障
    if let Some(notice) = crate::render::engine_notice(&config.agent) {
        runtime.record_meta(notice)?;
    }
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
                        runtime.record_meta(interrupted_failure_text(&error))?;
                    }
                } else if let Err(error) = outcome.result {
                    runtime.record_meta(turn_failure_text(&error))?;
                }
                if let Some(draft) = outcome.leftover_draft {
                    prefill = Some(draft);
                }
                drain_submission_queue(
                    paths,
                    &config,
                    &mut agent,
                    &mut runtime,
                    &mut mode,
                    &mut input_history,
                    transcript_options.reasoning_mode,
                    transcript_options.tool_call_mode,
                )
                .await?;
                if let Some((draft, clipboard)) = take_stream_draft_prefill(&mut runtime) {
                    prefill = Some(draft);
                    prefill_clipboard = Some(clipboard);
                }
                continue;
            }
            None => break,
        };
        apply_ready_tool_registry(&mut tool_warmup, &mut agent, mode, &mut runtime)?;
        let input = submission.raw_input.trim();
        let mut submitted_input = input.to_string();
        if super::repl_commands::is_repl_exit_command(input) {
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
                        // 帮助改为浮动居中面板展示，关闭后全量重放恢复底层内容
                        center_panel::show_center_panel(
                            t("Help", "帮助"),
                            &crate::control_commands::help_text(
                                crate::control_commands::ControlSurface::Repl,
                            ),
                        )?;
                        runtime.redraw()?;
                    }
                    crate::control_commands::ControlCommand::Context => {
                        match crate::control_commands::context_info_text(paths) {
                            Ok(info) => {
                                center_panel::show_center_panel(t("Context", "上下文"), &info)?;
                                runtime.redraw()?;
                            }
                            Err(err) => runtime.record_meta(err.to_string())?,
                        }
                    }
                    crate::control_commands::ControlCommand::New { title } => {
                        let message = crate::control_commands::create_new_session(paths, &title)?;
                        state = StateStore::new(paths)?;
                        state.init_files()?;
                        agent.replace_state(state.clone())?;
                        prefill = None;
                        runtime.clear()?;
                        runtime.record_meta(message)?;
                    }
                    crate::control_commands::ControlCommand::Resume { id } => {
                        let session_id = match id {
                            Some(id) => id,
                            None => {
                                let picked = sessions::select_session_id_interactively(paths);
                                // 内联选择 UI 退出后全量重放，清除其占用行的残留
                                runtime.redraw()?;
                                match picked {
                                    Ok(id) => id,
                                    Err(err) => {
                                        runtime.record_meta(err.to_string())?;
                                        continue;
                                    }
                                }
                            }
                        };
                        match crate::control_commands::resume_session(paths, &session_id) {
                            Ok(message) => {
                                state = StateStore::new(paths)?;
                                state.init_files()?;
                                agent.replace_state(state.clone())?;
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
                            // 用 raw 模式轮询按键取消，而不是 tokio 的 ctrl_c：
                            // 后者首次 poll 即永久接管 SIGINT，会让 cooked 段的
                            // Ctrl+C 行为在使用 /compact 前后不一致
                            let mut compact_guard = terminal_restore::TerminalInputGuard::enable(
                                &mut io::stdout(),
                                false,
                            )?;
                            let compact_result = loop {
                                tokio::select! {
                                    result = &mut compact => break result.map(|_| ()),
                                    _ = resize_tick.tick() => {
                                        let mut runtime_ref = runtime.borrow_mut();
                                        process_stream_tick(&mut *runtime_ref)?;
                                        drop(runtime_ref);
                                        if poll_compact_cancel()? {
                                            runtime.borrow_mut().record_meta(
                                                t("compaction cancelled", "已取消压缩")
                                                    .to_string(),
                                            )?;
                                            break Ok(());
                                        }
                                    }
                                }
                            };
                            let guard_result = compact_guard.finish(&mut io::stdout());
                            compact_result.and(guard_result)
                        };
                        runtime.finish_stream()?;
                        result?;
                    }
                    crate::control_commands::ControlCommand::Clear { all } => {
                        let message = crate::control_commands::clear_state(paths, all)?;
                        // 清理的是会话状态，跨会话输入历史保留
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
                            None => {
                                let picked = model_select::select_model_index_interactively(paths);
                                // 内联选择 UI 退出后全量重放，清除其占用行的残留
                                runtime.redraw()?;
                                match picked {
                                    Ok(index) => index,
                                    Err(err) => {
                                        runtime.record_meta(err.to_string())?;
                                        continue;
                                    }
                                }
                            }
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
                    crate::control_commands::ControlCommand::Tree { turn_id } => {
                        // 1. 未指定轮次时打开树界面交互选择
                        let target = match turn_id {
                            Some(id) => Some(id),
                            None => {
                                let picked = tree_select::select_turn_interactively(paths);
                                // 内联选择 UI 退出后全量重放，清除其占用行的残留
                                runtime.redraw()?;
                                match picked {
                                    Ok(target) => target,
                                    Err(err) => {
                                        runtime.record_meta(err.to_string())?;
                                        continue;
                                    }
                                }
                            }
                        };
                        let Some(target) = target else {
                            runtime.record_meta(
                                t("tree navigation cancelled", "已取消会话树切换").to_string(),
                            )?;
                            continue;
                        };
                        // 2. 切换活动叶子，随后清屏重放该分支历史
                        match agent.state().switch_active_leaf(&target) {
                            Ok(()) => {
                                runtime.clear()?;
                                record_repl_history(&mut runtime, agent.state())?;
                                runtime.record_meta(
                                    t("switched to the selected turn", "已切换到所选轮次")
                                        .to_string(),
                                )?;
                            }
                            Err(err) => runtime.record_meta(err.to_string())?,
                        }
                    }
                    crate::control_commands::ControlCommand::Agent { selection } => {
                        let selection = match selection {
                            Some(index) => Some(index),
                            None => {
                                let picked = agent_select::select_agent_index_interactively(paths);
                                // 内联选择 UI 退出后全量重放，清除其占用行的残留
                                runtime.redraw()?;
                                match picked {
                                    Ok(index) => index,
                                    Err(err) => {
                                        runtime.record_meta(err.to_string())?;
                                        continue;
                                    }
                                }
                            }
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
        if input.eq_ignore_ascii_case("/auto") || input.eq_ignore_ascii_case("/auto-audit") {
            mode = AgentMode::AutoAudit;
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
            // 面板内的操作失败只提示，不能让整个 REPL 退出
            if let Err(err) = run_repl_background_manager(paths, &config).await {
                runtime.record_meta(err.to_string())?;
            }
            continue;
        }
        // 剩余的 / 开头命令形态输入不再静默发给模型：拼错的命令、
        // 带多余参数的已知命令都在这里拦截并提示
        if let Some(hint) = unknown_slash_command_hint(input, config.agent.engine.is_external()) {
            runtime.record_meta(hint)?;
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
            // 落盘失败不影响本轮对话，历史只是辅助功能
            let _ = crate::state::input_history::append_input_history(paths, input);
        }
        if !goal_continuation {
            runtime.record_user(mode, input.to_string())?;
        }
        // 4. 模式变化时换工具表；每轮只做轻量 prepare
        if agent.mode() != mode {
            let registry = build_repl_tool_registry(&config, paths, mode)?;
            agent.switch_mode(mode, registry)?;
        }
        agent.prepare_for_turn()?;
        // 用户主动发话：清除历史未消费回执
        agent.discard_stale_external_completion_notices()?;
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
        apply_stream_mode(&runtime, &mut mode);
        if outcome.interrupted {
            if !state.latest_interrupted_turn_has_content(&submitted_input)? {
                prefill = Some(submitted_input);
            } else if let Some(draft) = outcome.leftover_draft {
                prefill = Some(draft);
            }
            // 中断后仍执行已入队内容
            drain_submission_queue(
                paths,
                &config,
                &mut agent,
                &mut runtime,
                &mut mode,
                &mut input_history,
                transcript_options.reasoning_mode,
                transcript_options.tool_call_mode,
            )
            .await?;
            if let Some((draft, clipboard)) = take_stream_draft_prefill(&mut runtime) {
                prefill = Some(draft);
                prefill_clipboard = Some(clipboard);
            }
            continue;
        }
        if let Err(error) = outcome.result {
            // 断连类错误保留可重试提示，其余错误展示完整错误链
            runtime.record_meta(turn_failure_text(&error))?;
            if crate::llm::is_transient_transport_error(&error) {
                prefill = Some(submitted_input.clone());
            } else if let Some(draft) = outcome.leftover_draft {
                prefill = Some(draft);
            }
            continue;
        }
        if let Some(draft) = outcome.leftover_draft {
            prefill = Some(draft);
        }
        // 1. 本轮成功后依次执行 Tab 入队的消息
        drain_submission_queue(
            paths,
            &config,
            &mut agent,
            &mut runtime,
            &mut mode,
            &mut input_history,
            transcript_options.reasoning_mode,
            transcript_options.tool_call_mode,
        )
        .await?;
        if let Some((draft, clipboard)) = take_stream_draft_prefill(&mut runtime) {
            prefill = Some(draft);
            prefill_clipboard = Some(clipboard);
        }
    }
    agent.shutdown_external_engine().await?;
    Ok(())
}

/// 轮询 /compact 执行期间的取消按键。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 收到 Ctrl+C 或 Esc 时返回 true
fn poll_compact_cancel() -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            let ctrl_c = matches!(key.code, KeyCode::Char('c'))
                && key.modifiers.contains(KeyModifiers::CONTROL);
            if ctrl_c || matches!(key.code, KeyCode::Esc) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
