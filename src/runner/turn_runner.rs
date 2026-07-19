use super::{
    AutomaticInputEvent, RunnerEvent, RunnerEventSink, SubmissionSource, UserInputSubmission,
};
use crate::agent::{Agent, ExternalEventBatch};
use crate::llm::ChatResult;
use anyhow::Result;
use std::collections::VecDeque;
use std::time::Instant;

/// 单轮 runner，当前只包装现有 Agent 单轮调用。
pub(crate) struct TurnRunner<'agent> {
    agent: &'agent mut Agent,
    wait_for_external_events: bool,
}

impl<'agent> TurnRunner<'agent> {
    /// 创建单轮 runner。
    ///
    /// 参数:
    /// - `agent`: 当前会话 Agent
    ///
    /// 返回:
    /// - 单轮 runner
    pub(crate) fn new(agent: &'agent mut Agent) -> Self {
        Self {
            agent,
            wait_for_external_events: true,
        }
    }

    /// 根据入口来源设置是否等待后台任务完成后自动续轮。
    ///
    /// 参数:
    /// - `agent`: 当前会话 Agent
    /// - `source`: submission 来源
    ///
    /// 返回:
    /// - 已配置外部事件等待策略的单轮 runner
    pub(crate) fn for_source(agent: &'agent mut Agent, source: SubmissionSource) -> Self {
        Self {
            agent,
            wait_for_external_events: source_waits_for_external_events(source),
        }
    }

    /// 执行用户输入单轮对话。
    ///
    /// 参数:
    /// - `input`: 用户输入 submission
    /// - `sink`: runner 事件接收器
    ///
    /// 返回:
    /// - 聊天结果
    pub(crate) async fn run_user_input<S>(
        &mut self,
        input: &UserInputSubmission,
        sink: &mut S,
    ) -> Result<ChatResult>
    where
        S: RunnerEventSink,
    {
        let mut queued_inputs = VecDeque::from([input.clone()]);
        let mut pending_external_events = None::<ExternalEventBatch>;
        loop {
            let mut current = queued_inputs
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("automatic input queue is empty"))?;
            // 1. 自动队列项每次读取最新目标，并在发送给模型前发布用户可见消息
            let automatic = current.automatic_input.take();
            if let Some(automatic) = automatic.as_ref() {
                let goal_state = self
                    .agent
                    .state()
                    .goal()?
                    .filter(|goal| goal.status.is_active());
                let goal = goal_state.as_ref();
                current.input = automatic
                    .prompt_text(goal)
                    .ok_or_else(|| anyhow::anyhow!("no active goal to continue"))?;
                current.image_urls.clear();
                current.turn_id = None;
                sink.on_runner_event(RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
                    automatic.kind,
                    automatic.display_text(goal),
                )))?;
            }
            let active_goal = self
                .agent
                .state()
                .goal()?
                .filter(|goal| goal.status.is_active());
            let started = Instant::now();
            let result = self
                .agent
                .chat_stream_with_images(
                    &current.input,
                    current.image_urls.clone(),
                    current.turn_id.clone(),
                    |event| sink.on_runner_event(RunnerEvent::Agent(event)),
                )
                .await;
            let elapsed = started.elapsed().as_secs().max(1);
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    if let Some(goal) = active_goal {
                        let _ = self
                            .agent
                            .state()
                            .account_goal_progress(&goal.id, 0, elapsed);
                        let _ = self
                            .agent
                            .state()
                            .set_goal_status(crate::goal::GoalStatus::Blocked);
                    }
                    return Err(error);
                }
            };
            // 2. 模型成功接收自动输入后再确认外部完成通知，失败时保留通知供下次重试
            if let Some(batch) = pending_external_events.take() {
                self.agent.acknowledge_external_events(&batch)?;
            }
            // 3. 只把本轮开始时已经活动的目标计入使用量，避免倒算创建目标之前的消耗
            if let Some(goal) = active_goal {
                let tokens = result
                    .usage
                    .as_ref()
                    .map(|usage| usage.total_tokens)
                    .unwrap_or_default();
                self.agent
                    .state()
                    .account_goal_progress(&goal.id, tokens, elapsed)?;
            }
            // 4. CLI 单次命令只返回当前模型结果，不等待后台工作完成
            if !self.wait_for_external_events {
                sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
                return Ok(result);
            }
            // 5. Goal 处于活动或阻塞状态时，等待后台工作完成并主动发起完整续轮
            if let Some(goal) = self.agent.state().goal()? {
                if goal.status.accepts_external_wake() {
                    let batch = self
                        .agent
                        .wait_for_goal_events(|| sink.on_runner_event(RunnerEvent::WaitingExternal))
                        .await?;
                    let latest_goal = self.agent.state().goal()?;
                    if let Some(batch) = batch {
                        if latest_goal
                            .as_ref()
                            .is_some_and(|goal| goal.status.accepts_external_wake())
                        {
                            if latest_goal
                                .as_ref()
                                .is_some_and(|goal| goal.status == crate::goal::GoalStatus::Blocked)
                            {
                                self.agent
                                    .state()
                                    .set_goal_status(crate::goal::GoalStatus::Active)?;
                            }
                            let prompt = batch.prompt().to_string();
                            let display = batch.display().to_string();
                            pending_external_events = Some(batch);
                            queued_inputs.push_back(
                                UserInputSubmission::new(String::new(), input.mode)
                                    .with_goal_event(prompt, display),
                            );
                            continue;
                        }
                    }
                    if latest_goal.is_some_and(|goal| goal.status.is_active()) {
                        queued_inputs.push_back(
                            UserInputSubmission::new(String::new(), input.mode)
                                .with_goal_continuation(),
                        );
                        continue;
                    }
                }
            }
            // 6. 非 Goal 会话同样等待未绑定 Goal 的后台工作，并通过自动队列发起新轮次
            if let Some(batch) = self
                .agent
                .wait_for_session_events(|| sink.on_runner_event(RunnerEvent::WaitingExternal))
                .await?
            {
                let prompt = batch.prompt().to_string();
                let display = batch.display().to_string();
                pending_external_events = Some(batch);
                queued_inputs.push_back(
                    UserInputSubmission::new(String::new(), input.mode)
                        .with_external_event(prompt, display),
                );
                continue;
            }
            sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
            return Ok(result);
        }
    }
}

/// 判断指定入口是否应该等待外部完成事件并自动续轮。
///
/// 参数:
/// - `source`: submission 来源
///
/// 返回:
/// - Web 和网关入口返回 `true`；TUI 使用独立外部事件监听器
fn source_waits_for_external_events(source: SubmissionSource) -> bool {
    !matches!(
        source,
        SubmissionSource::Command | SubmissionSource::Repl | SubmissionSource::ShellIntercept
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证一次性 CLI 命令不会等待后台工作完成。
    #[test]
    fn command_sources_skip_external_wait() {
        assert!(!source_waits_for_external_events(SubmissionSource::Command));
        assert!(!source_waits_for_external_events(
            SubmissionSource::ShellIntercept
        ));
    }

    /// 验证 TUI 单轮会先返回输入框，外部完成事件交给独立监听器。
    #[test]
    fn repl_source_returns_before_external_completion() {
        assert!(!source_waits_for_external_events(SubmissionSource::Repl));
    }

    /// 验证 Web 与网关持久入口仍会消费后台完成事件。
    #[test]
    fn persistent_sources_keep_external_wait() {
        assert!(source_waits_for_external_events(SubmissionSource::Web));
        assert!(source_waits_for_external_events(SubmissionSource::Gateway));
    }
}
