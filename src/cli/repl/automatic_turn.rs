use super::agent_slot::ReplAgentSlot;
use super::*;

/// 执行自动唤醒并接续队列；参数为宿主状态、唤醒与显示配置；返回是否已确认退出。
#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    paths: &SaiPaths,
    config: &AppConfig,
    state: &StateStore,
    agent: &mut ReplAgentSlot,
    runtime: &mut ReplRuntime,
    warmup: &mut ReplToolWarmup,
    external: &mut ReplExternalEvents,
    mode: &mut AgentMode,
    history: &mut Vec<crate::state::input_history::InputHistoryEntry>,
    options: render::transcript::TranscriptRenderOptions,
    wake: crate::agent::ExternalEventWake,
    prefill: &mut Option<String>,
    clipboard: &mut Option<super::super::repl_clipboard::ReplClipboardState>,
    exit_armed: &mut bool,
) -> Result<bool> {
    apply_ready_tool_registry(warmup, agent, *mode, runtime)?;
    let owner_key = state.state_dir().display().to_string();
    let context = ReplTurnContext {
        paths,
        config,
        owner_key: &owner_key,
    };
    let outcome = execute_automatic_repl_turn(
        context,
        agent,
        runtime,
        *mode,
        options.reasoning_mode,
        options.tool_call_mode,
        wake,
    )
    .await?;
    if outcome.detached {
        return Ok(false);
    }
    if outcome.exit_requested
        && confirm_repl_exit(paths, config, state, runtime, exit_armed).await?
    {
        return Ok(true);
    }
    if outcome.interrupted {
        if let Err(error) = outcome.result {
            runtime.record_failure(interrupted_failure_text(&error))?;
        }
    } else if let Err(error) = outcome.result {
        external.pause(agent.state().state_dir());
        runtime.record_failure(turn_failure_text(&error))?;
    }
    if let Some(draft) = outcome.leftover_draft {
        *prefill = Some(draft);
    }
    if runtime.has_ready_turn_submission(agent.state().goal()?.map(|goal| goal.status)) {
        external.resume();
    }
    let exit = drain_submission_queue(
        context,
        agent,
        runtime,
        mode,
        history,
        options.reasoning_mode,
        options.tool_call_mode,
    )
    .await?;
    if runtime.has_session_switch_request() {
        return Ok(false);
    }
    if let Some((draft, attachments)) = take_stream_draft_prefill(runtime) {
        *prefill = Some(draft);
        *clipboard = Some(attachments);
    }
    Ok(exit && confirm_repl_exit(paths, config, state, runtime, exit_armed).await?)
}
