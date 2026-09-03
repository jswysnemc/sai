use super::terminal_restore::restore_stream_terminal_modes;
use super::*;
use crate::agent::{Agent, AgentEvent, ExternalEventBatch, ExternalEventWake};
use crate::cli::repl_runtime::{StreamCommandContext, StreamInputAction};

/// 自动唤醒对应的 runner submission 与待确认事件批次。
pub(super) struct AutomaticReplSubmission {
    pub(super) submission: crate::runner::RunnerSubmission,
    pub(super) batch: Option<ExternalEventBatch>,
}

/// TUI 单轮执行结果。
pub(super) struct ReplTurnOutcome {
    pub(super) interrupted: bool,
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
            let input = crate::runner::UserInputSubmission::new(String::new(), mode)
                .with_external_event(batch.prompt().to_string(), batch.display().to_string());
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
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置
/// - `agent`: 当前复用 Agent
/// - `runtime`: TUI 运行期
/// - `mode`: 当前 Agent 模式
/// - `reasoning_mode`: 推理内容展示模式
/// - `tool_call_mode`: 工具调用展示模式
/// - `wake`: Goal 续轮或外部完成事件
/// - `event_bus`: 会话事件总线，转发给轮次执行
///
/// 返回:
/// - 自动轮次执行结果
pub(super) async fn execute_automatic_repl_turn(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    owner_key: &str,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    wake: ExternalEventWake,
    event_bus: Option<crate::runner::ActorHandle>,
) -> Result<ReplTurnOutcome> {
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
        event_bus,
    )
    .await?;
    // 中断同样算作已消费：用户按下 Ctrl+C 就是看到了这批回执并主动放弃。
    // 若此时不确认，下一次等待会立刻重投同一批，而 take_ready 又排在读键之前，
    // 用户既抢不回输入、也退不出去，形成中断→重投→再中断的活锁。
    // 仅在真正失败（provider 报错）时保留，留给下次重试。
    if outcome.interrupted || outcome.result.is_ok() {
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
/// - `event_bus`: 会话事件总线；持有者模式下把 RunnerEvent 广播给跟随端
///
/// 返回:
/// - 中断标志、退出请求与对话执行结果
pub(super) async fn execute_repl_turn(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    owner_key: &str,
    submission: crate::runner::RunnerSubmission,
    event_bus: Option<crate::runner::ActorHandle>,
) -> Result<ReplTurnOutcome> {
    let runner = crate::runner::SessionRunner::new(paths)
        .with_config(config.clone())
        .with_inter_message_source(runtime.inter_message_source());
    // 事件通道：sink 只负责入队，交互弹窗与 transcript 写入都由消费循环完成。
    // 用无界 std::sync::mpsc 而非有界通道——sink 是在异步轮次里被同步调用的，
    // 一旦通道满而阻塞就会卡住整个 tokio worker 线程；无界通道下由消费侧
    // 每 25ms 的排空节奏承担背压
    let (event_tx, event_rx) = std::sync::mpsc::channel::<crate::runner::RunnerEvent>();
    // 同一事件还要进会话事件总线：跟随端靠它实时看到本轮的流式输出与工具
    // 活动。总线只入队不 await，失败也不影响本地渲染——对端失联不该拖垮本轮
    let (run_input, run_images) = match submission.kind {
        crate::runner::RunnerSubmissionKind::UserInput(ref input) => {
            (input.input.clone(), input.image_urls.clone())
        }
        _ => (String::new(), Vec::new()),
    };
    if let Some(bus) = event_bus.as_ref() {
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let _ = bus.begin_run(&run_id, &run_input, &run_images);
    }
    let mut sink = |event: crate::runner::RunnerEvent| {
        event_tx
            .send(event.clone())
            .map_err(|_| anyhow::anyhow!("runner event channel is closed"))?;
        if let Some(bus) = event_bus.as_ref() {
            let _ = bus.publish(event);
        }
        Ok(())
    };
    let stream_mode = submission.mode;
    // 命令上下文必须在建 chat future 之前抓取：之后 Agent 被独占借用，
    // 立即执行的命令就拿不到 paths 与子智能体作用域键了
    let stream_ctx = StreamCommandContext::capture(paths, owner_key.to_string(), stream_mode);
    // 1. 本轮开始时保留可编辑输入框，供 Tab 入队
    runtime.begin_stream_composer(stream_mode)?;
    // 2. 绑定热切换句柄：Shift+Tab 立即改权限模式
    runtime.bind_live_mode(agent.live_mode_handle(), agent.session_id());
    // 停止标志在丢弃 chat future 前置位，轮次守卫据此把本轮记为用户中断
    let cancel_flag = agent.cancel_handle();
    let chat = runner.run_submission_with_agent(submission, agent, &mut sink);
    tokio::pin!(chat);
    let mut interrupted = false;
    let mut exit_requested = false;
    let mut resize_tick = tokio::time::interval(Duration::from_millis(25));
    resize_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // 流式阶段同样保持 bracketed paste 与键盘增强：否则粘贴多行会被
    // 拆成逐字符按键，其中的回车逐条入队自动发送；光标由渲染器管理
    let mut stream_terminal_guard =
        super::terminal_restore::TerminalInputGuard::enable(&mut io::stdout(), false)?;
    let result: Result<()> = async {
        loop {
            tokio::select! {
                result = &mut chat => {
                    let outcome = result.map(|_| ());
                    // 轮次结束后排空剩余事件：Completed / Failed / FinalSummary
                    // 是 sink 最后发出的，不排空就会丢掉收尾渲染
                    drain_runner_events(&event_rx, runtime)?;
                    break outcome;
                }
                _ = resize_tick.tick() => {
                    drain_runner_events(&event_rx, runtime)?;
                    process_stream_tick(runtime)?;
                    let action = process_stream_input(runtime, &stream_ctx)?;
                    if action != StreamInputAction::Continue {
                        // 先置位再跳出：跳出会丢弃 chat future，
                        // 守卫在析构时读取此标志才能把本轮记成中断而非失败。
                        // 退出同样走中断路径，由主循环收尾后 break；
                        // 直接 std::process::exit 会跳过终端模式恢复
                        cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        interrupted = true;
                        exit_requested = action == StreamInputAction::Exit;
                        // 中断前也排空：已经产出但还没渲染的输出应当保留
                        drain_runner_events(&event_rx, runtime)?;
                        break Ok(());
                    }
                }
            }
        }
    }
    .await;
    // 终端模式恢复失败也不能跳过流状态清理，先记录结果最后上报
    let guard_result = stream_terminal_guard.finish(&mut io::stdout());
    let leftover_draft = {
        runtime.clear_live_mode();
        runtime.finish_stream()?;
        let draft = runtime.stream_draft().text.trim().to_string();
        (!draft.is_empty()).then_some(draft)
    };
    guard_result?;
    // 1. 答复结束（完成 / 中断 / 失败）发送桌面通知
    let body = if interrupted {
        crate::i18n::text("Reply interrupted", "答复已中断")
    } else if result.is_err() {
        crate::i18n::text("Reply failed", "答复失败")
    } else {
        crate::i18n::text("Reply complete", "答复已完成")
    };
    crate::reply_notify::notify_reply_complete(config, "Sai", body);
    Ok(ReplTurnOutcome {
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
) -> Result<()> {
    while let Ok(event) = rx.try_recv() {
        handle_turn_event(event, runtime)?;
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
        runtime.pause_for_permission_prompt()?;
        runtime.record_permission_request(request.clone())?;
        prompt_permission_request_tui(request, runtime)?;
        restore_stream_terminal_modes()?;
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
                prompt_ssh_secret_request_tui(&request, runtime)?;
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
            ExternalEventWake::Completion(batch),
            AgentMode::Yolo,
            render::ReasoningDisplayMode::Summary,
            render::ToolCallDisplayMode::Summary,
            render::StreamRenderOptions::default(),
        );

        assert!(automatic.batch.is_some());
        assert!(matches!(
            automatic.submission.kind,
            crate::runner::RunnerSubmissionKind::UserInput(crate::runner::UserInputSubmission {
                automatic_input: Some(_),
                ..
            })
        ));
    }
}
