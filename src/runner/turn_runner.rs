use super::{RunnerEvent, RunnerEventSink, SubmissionSource, UserInputSubmission};
use crate::agent::{Agent, InterMessageSource};
use crate::llm::ChatResult;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

/// 单轮 runner，当前只包装现有 Agent 单轮调用。
pub(crate) struct TurnRunner<'agent> {
    agent: &'agent mut Agent,
    wait_for_external_events: bool,
    inter_message_source: Option<Arc<dyn InterMessageSource>>,
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
            inter_message_source: None,
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
            inter_message_source: None,
        }
    }

    /// 设置当前活动回合可以消费的排队消息来源。
    ///
    /// 参数:
    /// - `source`: 可选排队消息来源
    ///
    /// 返回:
    /// - 更新后的 runner
    pub(crate) fn with_inter_message_source(
        mut self,
        source: Option<Arc<dyn InterMessageSource>>,
    ) -> Self {
        self.inter_message_source = source;
        self
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
        let active_goal = self
            .agent
            .state()
            .goal()?
            .filter(|goal| goal.status.is_active());
        let started = Instant::now();
        // 1. 本轮耗时从首次思考或正文输出开始；没有输出时从请求开始
        let first_output = std::sync::Arc::new(std::sync::Mutex::new(None::<Instant>));
        let result = {
            let first_output_cb = std::sync::Arc::clone(&first_output);
            self.agent
                .chat_stream_with_images_and_inter_messages(
                    &input.input,
                    input.image_urls.clone(),
                    input.turn_id.clone(),
                    self.inter_message_source.clone(),
                    self.wait_for_external_events,
                    |event| {
                        if matches!(&event, crate::agent::AgentEvent::Chunk(_)) {
                            let mut guard = first_output_cb.lock().unwrap();
                            if guard.is_none() {
                                *guard = Some(Instant::now());
                            }
                        }
                        sink.on_runner_event(RunnerEvent::Agent(event))
                    },
                )
                .await
        };
        let output_started = first_output.lock().unwrap().unwrap_or(started);
        let duration_ms = output_started.elapsed().as_millis() as u64;
        let elapsed = started.elapsed().as_secs().max(1);
        let mut result = match result {
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
        result.duration_ms = duration_ms.max(1);
        // 2. 将处理耗时写入会话数据库，供时间线恢复展示
        let _ = self
            .agent
            .state()
            .set_last_turn_duration_ms(result.duration_ms);
        // 3. 只把本轮开始时已经活动的目标计入使用量
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
        sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
        Ok(result)
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
