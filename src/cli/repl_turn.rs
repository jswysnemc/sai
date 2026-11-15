use super::terminal_restore::restore_stream_terminal_modes;
use super::*;
use crate::agent::{AgentEvent, ExternalEventBatch, ExternalEventWake};
use crate::cli::repl::agent_slot::ReplAgentSlot;
use crate::cli::repl_runtime::{StreamCommandContext, StreamInputAction};
use crate::cli::repl_turn_context::ReplTurnContext;

/// 自动唤醒对应的 runner submission 与待确认事件批次。
pub(super) struct AutomaticReplSubmission {
    pub(super) submission: crate::runner::RunnerSubmission,
    pub(super) batch: Option<ExternalEventBatch>,
}

/// TUI 单轮执行结果。
pub(super) struct ReplTurnOutcome {
    pub(super) interrupted: bool,
    /// 切换前台后任务仍运行，调用方不得访问 Agent 或执行旧会话收尾。
    pub(super) detached: bool,
    pub(super) result: Result<()>,
    /// 运行期间未入队的草稿，交回空闲输入框
    pub(super) leftover_draft: Option<String>,
    /// 运行期间用户输入了退出命令，主循环应立即退出 REPL
    pub(super) exit_requested: bool,
}

/// 把后台唤醒事件构造成带蓝色自动消息的 REPL submission。
///
/// 参数:
/// - `wake`: Goal 续轮或外部完成事件
/// - `mode`: 当前 Agent 模式
/// - `reasoning_mode`: 推理内容展示模式
/// - `tool_call_mode`: 工具调用展示模式
/// - `render_options`: 流式渲染配置
///
/// 返回:
/// - runner submission 与模型成功消费后需要确认的批次
pub(super) fn automatic_repl_submission(
    wake: ExternalEventWake,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    render_options: render::StreamRenderOptions,
) -> AutomaticReplSubmission {
    let (input, batch) = match wake {
        ExternalEventWake::GoalContinuation => (
            crate::runner::UserInputSubmission::new(String::new(), mode).with_goal_continuation(),
            None,
        ),
        ExternalEventWake::Completion(batch) => {
            let batch = *batch;
            let input = crate::runner::UserInputSubmission::new(String::new(), mode)
                .with_external_event_batch(batch.clone());
            (input, Some(batch))
        }
    };
    let submission =
        crate::runner::RunnerSubmission::user_input(crate::runner::SubmissionSource::Repl, input)
            .with_render_policy(crate::runner::RenderPolicy::new(
                false,
                reasoning_mode,
                tool_call_mode,
                render_options,
            ));
    AutomaticReplSubmission { submission, batch }
}

/// 执行一条 TUI 自动唤醒轮次并在成功后确认外部完成事件。
///
/// 参数:
/// - `context`: 应用路径、配置和当前会话的所有权键
/// - `agent`: 当前复用 Agent
/// - `runtime`: TUI 运行期
/// - `mode`: 当前 Agent 模式
/// - `reasoning_mode`: 推理内容展示模式
/// - `tool_call_mode`: 工具调用展示模式
/// - `wake`: Goal 续轮或外部完成事件
///
/// 返回:
/// - 自动轮次执行结果
pub(super) async fn execute_automatic_repl_turn(
    context: ReplTurnContext<'_>,
    agent: &mut ReplAgentSlot,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    wake: ExternalEventWake,
) -> Result<ReplTurnOutcome> {
    let ReplTurnContext {
        paths,
        config,
        owner_key,
    } = context;
    if agent.installed_mode() != mode {
        let registry = build_repl_tool_registry(config, paths, mode)?;
        agent.switch_mode(mode, registry)?;
    }
    agent.prepare_for_turn()?;
    let automatic = automatic_repl_submission(
        wake,
        mode,
        reasoning_mode,
        tool_call_mode,
        stream_render_options(config),
    );
    let batch = automatic.batch;
    let outcome = execute_repl_turn(
        paths,
        config,
        agent,
        runtime,
        owner_key,
        automatic.submission,
    )
    .await?;
    // 中断同样算作已消费：用户按下 Ctrl+C 就是看到了这批回执并主动放弃。
    // 若此时不确认，下一次等待会立刻重投同一批，而 take_ready 又排在读键之前，
    // 用户既抢不回输入、也退不出去，形成中断→重投→再中断的活锁。
    // 请求成功时由统一消息间隙确认；这里只处理用户主动中断，失败保留待重试
    if outcome.detached {
        if let Some(task) = agent.task.as_mut() {
            task.interrupted_batch = batch;
        }
        return Ok(outcome);
    }
    if outcome.interrupted {
        if let Some(batch) = batch.as_ref() {
            let _ = agent.acknowledge_external_events(batch);
        }
    }
    Ok(outcome)
}

/// 执行一条 TUI submission，并在运行期间维护流式渲染与输入缓存。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置
/// - `agent`: 当前复用 Agent
/// - `runtime`: TUI 运行期
/// - `owner_key`: 当前会话的子智能体作用域键
/// - `submission`: 用户或自动输入 submission
///
/// 返回:
/// - 中断标志、退出请求与对话执行结果
pub(super) async fn execute_repl_turn(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut ReplAgentSlot,
    runtime: &mut ReplRuntime,
    owner_key: &str,
    submission: crate::runner::RunnerSubmission,
) -> Result<ReplTurnOutcome> {
    // 1. 【终端队列】【主动继续】普通提交恢复队列，自动续轮和后台回执不解除失败暂停
    runtime.resume_queue_for_submission(&submission);
    let stream_mode = submission.mode;
    runtime.begin_stream_composer(stream_mode)?;
    let owned = agent
        .agent
        .take()
        .ok_or_else(|| anyhow::anyhow!("session already running"))?;
    agent.task = Some(super::repl_turn_task::ReplTurnTask::start(
        paths,
        config,
        owned,
        submission,
        runtime.inter_message_source(),
        runtime.session_event_sender(),
    )?);
    resume_repl_turn(paths, agent, runtime, owner_key).await
}

/// 接管原轮次的事件流与可编辑输入；参数为路径、Agent槽、UI与会话键；返回完成或脱离结果。
pub(super) async fn resume_repl_turn(
    paths: &SaiPaths,
    agent: &mut ReplAgentSlot,
    runtime: &mut ReplRuntime,
    owner_key: &str,
) -> Result<ReplTurnOutcome> {
    let task = agent
        .task
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("no running turn"))?;
    let final_mode_handle = task.live_mode.clone();
    runtime.bind_live_mode(final_mode_handle.clone(), &task.session_id);
    runtime.bind_live_model_settings(task.live_model_settings.clone());
    let stream_ctx =
        StreamCommandContext::capture(paths, owner_key.to_owned(), final_mode_handle.get());
    let mut interrupted = false;
    let mut exit_requested = false;
    let mut detached = false;
    let mut returned_agent = None;
    let mut pager: Option<crate::cli::repl_pager::PagerScreen> = None;
    let mut resize_tick = tokio::time::interval(Duration::from_millis(25));
    resize_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // 流式阶段同样保持 bracketed paste 与键盘增强：否则粘贴多行会被
    // 拆成逐字符按键，其中的回车逐条入队自动发送；光标由渲染器管理
    let mut stream_terminal_guard = runtime.acquire_input_guard(&mut io::stdout(), false)?;
    let result: Result<()> = async {
        loop {
            tokio::select! {
                result = &mut task.handle => {
                    let outcome = match result {
                        Ok((owned, result, stopped)) => {
                            returned_agent = Some(owned);
                            interrupted |= stopped;
                            result
                        }
                        Err(error) => Err(anyhow::anyhow!("session runner task failed: {error}")),
                    };
                    // 轮次结束后排空剩余事件：Completed / Failed / FinalSummary
                    // 是 sink 最后发出的，不排空就会丢掉收尾渲染。
                    // 先排空再关副屏，这样离开时的整屏重放已经包含收尾内容
                    drain_runner_events(&task.events, runtime, &mut pager)?;
                    close_pager(&mut pager, runtime)?;
                    break outcome;
                }
                _ = resize_tick.tick() => {
                    if runtime.has_session_switch_request() {
                        close_pager(&mut pager, runtime)?;
                        detached = true;
                        break Ok(());
                    }
                    drain_runner_events(&task.events, runtime, &mut pager)?;
                    if pager.is_some() {
                        let closed = match pager.as_mut() {
                            Some(screen) => screen.poll(Some(Duration::from_millis(0)), |focus, width| {
                                runtime.pager_view(focus, width)
                            })?,
                            None => false,
                        };
                        if closed {
                            close_pager(&mut pager, runtime)?;
                        }
                        // 副屏打开时不重绘主界面，也不读输入框按键
                        continue;
                    }
                    process_stream_tick(runtime)?;
                    let action = process_stream_input(runtime, &stream_ctx)?;
                    if action == StreamInputAction::SwitchSession || runtime.has_session_switch_request() {
                        close_pager(&mut pager, runtime)?;
                        detached = true;
                        break Ok(());
                    }
                    if action == StreamInputAction::ToggleFullscreen {
                        // Ctrl+O：切换全屏会话视图，不中断当前轮次
                        runtime.toggle_fullscreen()?;
                        continue;
                    }
                    if action != StreamInputAction::Continue {
                        // 先置位再跳出：跳出会丢弃 chat future，
                        // 守卫在析构时读取此标志才能把本轮记成中断而非失败。
                        // 退出同样走中断路径，由主循环收尾后 break；
                        // 直接 std::process::exit 会跳过终端模式恢复
                        task.interrupt();
                        interrupted = true;
                        exit_requested = action == StreamInputAction::Exit;
                        // 中断前也排空：已经产出但还没渲染的输出应当保留
                        drain_runner_events(&task.events, runtime, &mut pager)?;
                        close_pager(&mut pager, runtime)?;
                        // 取消分支只丢弃本轮 future，任务仍将原 Agent 交回。
                        match (&mut task.handle).await {
                            Ok((owned, result, _)) => {
                                returned_agent = Some(owned);
                                break result;
                            }
                            Err(error) => break Err(anyhow::anyhow!("session runner task failed: {error}")),
                        }
                    }
                }
            }
        }
    }
    .await;
    if detached {
        stream_terminal_guard.finish(&mut io::stdout())?;
        return Ok(ReplTurnOutcome {
            interrupted: false,
            detached: true,
            result: Ok(()),
            leftover_draft: None,
            exit_requested: false,
        });
    }
    if returned_agent.is_none() && !task.handle.is_finished() {
        task.interrupt();
        if let Ok((owned, _, _)) = (&mut task.handle).await {
            returned_agent = Some(owned);
        }
    }
    if interrupted {
        if let (Some(owned), Some(batch)) =
            (returned_agent.as_mut(), task.interrupted_batch.as_ref())
        {
            let _ = owned.acknowledge_external_events(batch);
        }
    }
    agent.task = None;
    agent.agent = returned_agent;
    if agent.agent.is_none() {
        anyhow::bail!("session runner terminated without returning its Agent");
    }
    // 2. 【终端队列】【失败暂停】先暂停投递再恢复终端，避免空闲循环立即开始剩余消息
    if interrupted || result.is_err() {
        runtime.pause_submission_queue();
    }
    if pager.is_some() || runtime.overlay_open() {
        if let Some(mut screen) = pager.take() {
            let _ = screen.leave();
        }
        let _ = runtime.resume_after_pager();
    }
    // 终端模式恢复失败也不能跳过流状态清理，先记录结果最后上报
    let guard_result = stream_terminal_guard.finish(&mut io::stdout());
    let leftover_draft = {
        runtime.stream_draft_mut().mode = Some(final_mode_handle.get());
        runtime.clear_live_mode();
        runtime.finish_stream()?;
        let draft = runtime.stream_draft().text.trim().to_string();
        (!draft.is_empty()).then_some(draft)
    };
    guard_result?;
    Ok(ReplTurnOutcome {
        detached: false,
        interrupted,
        result,
        leftover_draft,
        exit_requested,
    })
}

/// 排空事件通道里积压的轮次事件。
///
/// 参数:
/// - `rx`: 事件接收端
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 处理结果
fn drain_runner_events(
    rx: &std::sync::mpsc::Receiver<crate::runner::RunnerEvent>,
    runtime: &mut ReplRuntime,
    pager: &mut Option<crate::cli::repl_pager::PagerScreen>,
) -> Result<()> {
    runtime.tick_session_events()?;
    while let Ok(event) = rx.try_recv() {
        if turn_event_needs_foreground(&event) {
            close_pager(pager, runtime)?;
        }
        handle_turn_event(event, runtime)?;
    }
    Ok(())
}

/// 权限、提问和 SSH 口令必须画在主屏上，否则会盖在副屏里且用户看不到。
pub(super) fn turn_event_needs_foreground(event: &crate::runner::RunnerEvent) -> bool {
    match event {
        crate::runner::RunnerEvent::Agent(AgentEvent::PermissionRequested(_))
        | crate::runner::RunnerEvent::Agent(AgentEvent::QuestionRequested(_)) => true,
        crate::runner::RunnerEvent::Agent(
            AgentEvent::ToolProgress { message, .. }
            | AgentEvent::ToolProgressIdentified { message, .. },
        ) => crate::ssh::is_secret_marker(message),
        _ => false,
    }
}

/// 离开副屏并补绘主界面。不设置取消标志。
fn close_pager(
    pager: &mut Option<crate::cli::repl_pager::PagerScreen>,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    if let Some(mut screen) = pager.take() {
        screen.leave()?;
        runtime.resume_after_pager()?;
    }
    Ok(())
}

/// 处理单个轮次事件。
///
/// 交互式事件（权限、提问、SSH 秘密）在这里弹窗，而不是在 sink 内部——
/// 三类答复都经 `request.id` 关联的全局状态回传（`decide_permission` /
/// `resolve_question` / `submit_secret`），因此弹窗发生在哪一侧都能送达，
/// Agent 只管 await 自己的一次性通道。
///
/// 参数:
/// - `event`: 轮次事件
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 处理结果
fn handle_turn_event(event: crate::runner::RunnerEvent, runtime: &mut ReplRuntime) -> Result<()> {
    if let crate::runner::RunnerEvent::Agent(AgentEvent::PermissionRequested(request)) = &event {
        // 1. 先清掉 working 动效，再挂权限控件，避免最后一行重叠
        runtime.begin_permission_prompt(request.clone())?;
    } else if let crate::runner::RunnerEvent::Agent(AgentEvent::QuestionRequested(pending)) = &event
    {
        prompt_question_request_tui(pending, runtime)?;
        restore_stream_terminal_modes()?;
    } else if let crate::runner::RunnerEvent::Agent(
        AgentEvent::ToolProgress { message, .. }
        | AgentEvent::ToolProgressIdentified { message, .. },
    ) = &event
    {
        // SSH 秘密交互标记：弹出安全输入界面，标记本身不进入 transcript，
        // 否则口令会落进历史区
        if crate::ssh::is_secret_marker(message) {
            if let Some(request) = crate::ssh::decode_progress_marker(message) {
                runtime.pause_for_permission_prompt()?;
                super::ssh_prompt::prompt_ssh_secret_request_tui(&request, runtime)?;
                restore_stream_terminal_modes()?;
            }
            return Ok(());
        }
    }
    runtime.record_runner_event(&event)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证外部完成事件构造成自动输入并保留待确认批次。
    #[test]
    fn external_wake_builds_automatic_repl_submission() {
        let batch = crate::agent::ExternalEventBatch::for_test(
            "<external-completion-events>done</external-completion-events>",
            "后台工作已完成",
        );
        let automatic = automatic_repl_submission(
            ExternalEventWake::completion(batch),
            AgentMode::Yolo,
            render::ReasoningDisplayMode::Summary,
            render::ToolCallDisplayMode::Summary,
            render::StreamRenderOptions::default(),
        );

        assert!(automatic.batch.is_some());
        assert!(matches!(
            automatic.submission.kind,
            crate::runner::RunnerSubmissionKind::UserInput(input) if input.automatic_input.is_some()
        ));
    }
}
