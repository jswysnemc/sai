use super::*;
use crate::agent::{Agent, AgentEvent, ExternalEventBatch, ExternalEventWake};

/// 自动唤醒对应的 runner submission 与待确认事件批次。
pub(super) struct AutomaticReplSubmission {
    pub(super) submission: crate::runner::RunnerSubmission,
    pub(super) batch: Option<ExternalEventBatch>,
}

/// TUI 单轮执行结果。
pub(super) struct ReplTurnOutcome {
    pub(super) interrupted: bool,
    pub(super) result: Result<()>,
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
///
/// 返回:
/// - 自动轮次执行结果
pub(super) async fn execute_automatic_repl_turn(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    reasoning_mode: render::ReasoningDisplayMode,
    tool_call_mode: render::ToolCallDisplayMode,
    wake: ExternalEventWake,
) -> Result<ReplTurnOutcome> {
    if agent.mode() != mode {
        let registry = build_repl_tool_registry(config, paths, mode)?;
        agent.switch_mode(mode, registry);
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
    let outcome = execute_repl_turn(paths, config, agent, runtime, automatic.submission).await?;
    if !outcome.interrupted && outcome.result.is_ok() {
        if let Some(batch) = batch.as_ref() {
            agent.acknowledge_external_events(batch)?;
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
/// - `submission`: 用户或自动输入 submission
///
/// 返回:
/// - 中断标志与对话执行结果
pub(super) async fn execute_repl_turn(
    paths: &SaiPaths,
    config: &AppConfig,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    submission: crate::runner::RunnerSubmission,
) -> Result<ReplTurnOutcome> {
    let runner = crate::runner::SessionRunner::new(paths).with_config(config.clone());
    let runtime = std::cell::RefCell::new(runtime);
    let mut sink = |event: crate::runner::RunnerEvent| {
        if let crate::runner::RunnerEvent::Agent(AgentEvent::PermissionRequested(request)) = &event
        {
            runtime
                .borrow_mut()
                .record_permission_request(request.clone())?;
            prompt_permission_request_tui(request, &runtime)?;
            crossterm::terminal::enable_raw_mode()?;
        }
        if let crate::runner::RunnerEvent::Agent(AgentEvent::QuestionRequested(pending)) = &event {
            prompt_question_request_tui(pending, &runtime)?;
            crossterm::terminal::enable_raw_mode()?;
        }
        runtime.borrow_mut().record_runner_event(&event)
    };
    let chat = runner.run_submission_with_agent(submission, agent, &mut sink);
    tokio::pin!(chat);
    let mut interrupted = false;
    let mut resize_tick = tokio::time::interval(Duration::from_millis(25));
    resize_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    crossterm::terminal::enable_raw_mode()?;
    let result: Result<()> = async {
        loop {
            tokio::select! {
                result = &mut chat => break result.map(|_| ()),
                _ = resize_tick.tick() => {
                    let mut runtime_ref = runtime.borrow_mut();
                    process_stream_tick(&mut runtime_ref)?;
                    if process_stream_input(&mut runtime_ref)? {
                        interrupted = true;
                        break Ok(());
                    }
                }
            }
        }
    }
    .await;
    crossterm::terminal::disable_raw_mode()?;
    runtime.borrow_mut().finish_stream()?;
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
    })
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
