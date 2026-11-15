use super::agent_slot::ReplAgentSlot;
use super::*;
use std::collections::HashMap;

/// 离开前台的会话执行环境；Agent 和工具预热不因点击而重建。
pub(super) struct ParkedSession {
    state: StateStore,
    agent: ReplAgentSlot,
    warmup: ReplToolWarmup,
    mode: AgentMode,
}

/// 同工作区切换真正的执行环境；参数为目标ID、当前宿主状态和停放表；返回切换结果。
#[allow(clippy::too_many_arguments)]
pub(super) fn switch(
    paths: &SaiPaths,
    config: &AppConfig,
    client: &OpenAiCompatibleClient,
    target: &str,
    state: &mut StateStore,
    agent: &mut ReplAgentSlot,
    warmup: &mut ReplToolWarmup,
    mode: &mut AgentMode,
    runtime: &mut ReplRuntime,
    parked: &mut HashMap<String, ParkedSession>,
    prefill: &mut Option<String>,
    clipboard: &mut Option<super::super::repl_clipboard::ReplClipboardState>,
) -> Result<()> {
    if target == state.session_id() {
        return Ok(());
    }
    let workspace = crate::runtime_cwd::current_dir()?;
    let next = if let Some(cached) = parked.remove(target) {
        cached
    } else {
        let next_state = StateStore::for_workspace_session(paths, &workspace, target)?;
        next_state.init_files()?;
        let registry = build_repl_tool_registry_without_mcp_for_session(
            config,
            paths,
            *mode,
            next_state.session_id(),
            next_state.state_dir(),
        )?;
        let next_agent = Agent::new(
            config.clone(),
            paths,
            next_state.clone(),
            client.clone(),
            registry,
            *mode,
        )?;
        let next_warmup = ReplToolWarmup::start(
            config.clone(),
            paths.clone(),
            *mode,
            next_state.session_id().to_owned(),
            next_state.state_dir().to_owned(),
        );
        ParkedSession {
            state: next_state,
            agent: ReplAgentSlot::new(next_agent),
            warmup: next_warmup,
            mode: *mode,
        }
    };
    if let Err(error) =
        crate::state::switch_workspace_session(paths, &workspace, next.state.session_id())
    {
        parked.insert(target.to_owned(), next);
        return Err(error);
    }
    let previous_mode = agent
        .task
        .as_ref()
        .map(|task| task.live_mode.get())
        .unwrap_or(*mode);
    let restored = match runtime.switch_session_workspace(paths, &next.state) {
        Ok(restored) => restored,
        Err(error) => {
            let _ = crate::state::switch_workspace_session(paths, &workspace, state.session_id());
            parked.insert(target.to_owned(), next);
            return Err(error);
        }
    };
    let old_key = state.session_id().to_owned();
    let old = ParkedSession {
        state: std::mem::replace(state, next.state),
        agent: std::mem::replace(agent, next.agent),
        warmup: std::mem::replace(warmup, next.warmup),
        mode: {
            *mode = next.mode;
            previous_mode
        },
    };
    parked.insert(old_key, old);
    *prefill = None;
    *clipboard = None;
    if !restored {
        record_repl_history(runtime, state)?;
    }
    if !agent.is_running() {
        let draft = runtime.take_session_input_draft();
        runtime.save_footer_draft(draft);
    }
    Ok(())
}

/// 退出时取消本终端仍运行的轮次并恰好关闭一次插件；参数为前台槽和停放表；返回无。
pub(super) async fn finish_agents(
    mut current: ReplAgentSlot,
    parked: HashMap<String, ParkedSession>,
) {
    let mut slots = vec![&mut current];
    let mut parked: Vec<_> = parked.into_values().collect();
    slots.extend(parked.iter_mut().map(|session| &mut session.agent));
    for slot in slots {
        if let Some(mut task) = slot.task.take() {
            task.interrupt();
            if let Ok((owned, _, _)) = task.handle.await {
                slot.agent = Some(owned);
            }
        }
        if let Some(agent) = slot.agent.as_mut() {
            agent.finish_hooks().await;
        }
    }
}

/// 恢复命令优先复用同工作区任务；参数为恢复目标和宿主状态；返回恢复结果，跨工作区不复用执行槽。
#[allow(clippy::too_many_arguments)]
pub(super) fn resume(
    paths: &SaiPaths,
    config: &AppConfig,
    client: &OpenAiCompatibleClient,
    target: &crate::state::ResumeTarget,
    state: &mut StateStore,
    agent: &mut ReplAgentSlot,
    warmup: &mut ReplToolWarmup,
    mode: &mut AgentMode,
    runtime: &mut ReplRuntime,
    parked: &mut HashMap<String, ParkedSession>,
    prefill: &mut Option<String>,
    clipboard: &mut Option<super::super::repl_clipboard::ReplClipboardState>,
) -> Result<()> {
    let directory = target.directory(paths)?;
    if directory == crate::runtime_cwd::current_dir()? {
        return switch(
            paths,
            config,
            client,
            &target.session.info.id,
            state,
            agent,
            warmup,
            mode,
            runtime,
            parked,
            prefill,
            clipboard,
        );
    }
    if !parked.is_empty() {
        anyhow::bail!("请先结束本终端停放的会话，再恢复其他工作区会话");
    }
    let next = session_resume::prepare(paths, config, client, *mode, target)?;
    *state = next.state;
    *agent = ReplAgentSlot::new(next.agent);
    *warmup = next.warmup;
    *prefill = None;
    *clipboard = None;
    runtime.clear()?;
    runtime.initialize_session_workspace(state);
    runtime.record_meta(crate::cli::session_resume::message(target))?;
    record_repl_history(runtime, state)
}

/// 接回运行会话并处理结束；参数为前台状态及输入草稿；返回空表示再次脱离、true表示退出。
#[allow(clippy::too_many_arguments)]
pub(super) async fn resume_running(
    paths: &SaiPaths,
    config: &AppConfig,
    state: &StateStore,
    agent: &mut ReplAgentSlot,
    runtime: &mut ReplRuntime,
    mode: &mut AgentMode,
    prefill: &mut Option<String>,
    clipboard: &mut Option<super::super::repl_clipboard::ReplClipboardState>,
    exit_armed: &mut bool,
) -> Result<Option<bool>> {
    let owner_key = state.state_dir().display().to_string();
    let outcome =
        super::super::repl_turn::resume_repl_turn(paths, agent, runtime, &owner_key).await?;
    if outcome.detached {
        return Ok(None);
    }
    apply_stream_mode(runtime, mode);
    if let Err(error) = outcome.result {
        runtime.record_failure(turn_failure_text(&error))?;
    }
    if outcome.exit_requested
        && confirm_repl_exit(paths, config, state, runtime, exit_armed).await?
    {
        return Ok(Some(true));
    }
    if let Some((text, attachments)) = take_stream_draft_prefill(runtime) {
        *prefill = Some(text);
        *clipboard = Some(attachments);
    }
    Ok(Some(false))
}

/// 新建后切换执行槽，保留旧会话Agent；参数为标题与宿主状态；返回创建切换结果。
#[allow(clippy::too_many_arguments)]
pub(super) fn new_session(
    paths: &SaiPaths,
    config: &AppConfig,
    client: &OpenAiCompatibleClient,
    title: &str,
    state: &mut StateStore,
    agent: &mut ReplAgentSlot,
    warmup: &mut ReplToolWarmup,
    mode: &mut AgentMode,
    runtime: &mut ReplRuntime,
    parked: &mut HashMap<String, ParkedSession>,
    prefill: &mut Option<String>,
    clipboard: &mut Option<super::super::repl_clipboard::ReplClipboardState>,
) -> Result<()> {
    let next = crate::state::create_session_detached(paths, Some(title))?;
    switch(
        paths, config, client, &next.id, state, agent, warmup, mode, runtime, parked, prefill,
        clipboard,
    )?;
    runtime.record_meta(format!(
        "{}: {}",
        t("created session", "已创建会话"),
        next.id
    ))
}
