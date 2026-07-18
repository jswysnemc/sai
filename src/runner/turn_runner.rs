use super::{RunnerEvent, RunnerEventSink, UserInputSubmission};
use crate::agent::{Agent, GoalEventBatch};
use crate::llm::ChatResult;
use anyhow::Result;
use std::time::Instant;

/// 单轮 runner，当前只包装现有 Agent 单轮调用。
pub(crate) struct TurnRunner<'agent> {
    agent: &'agent mut Agent,
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
        Self { agent }
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
        let mut current = input.clone();
        let mut pending_external_events = None::<GoalEventBatch>;
        loop {
            // 1. 内部续轮每次读取最新目标，保证暂停、编辑和预算状态立即生效
            if current.goal_continuation {
                let goal = self
                    .agent
                    .state()
                    .goal()?
                    .filter(|goal| goal.status.is_active())
                    .ok_or_else(|| anyhow::anyhow!("no active goal to continue"))?;
                current.input = crate::goal::continuation_prompt(&goal);
                if let Some(prompt) = current.goal_event_prompt.take() {
                    current.input.push_str("\n\n");
                    current.input.push_str(&prompt);
                }
                current.image_urls.clear();
                current.turn_id = None;
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
            // 2. 只把本轮开始时已经活动的目标计入使用量，避免倒算创建目标之前的消耗
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
            if let Some(batch) = pending_external_events.take() {
                self.agent.acknowledge_goal_events(&batch)?;
            }
            // 3. Goal 处于活动或阻塞状态时，等待后台工作完成并主动发起完整续轮
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
                            pending_external_events = Some(batch);
                            current = UserInputSubmission::new(String::new(), input.mode)
                                .with_goal_continuation()
                                .with_goal_event_prompt(prompt);
                            continue;
                        }
                    }
                    if latest_goal.is_some_and(|goal| goal.status.is_active()) {
                        current = UserInputSubmission::new(String::new(), input.mode)
                            .with_goal_continuation();
                        continue;
                    }
                }
            }
            sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
            return Ok(result);
        }
    }
}
