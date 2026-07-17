use super::{RunnerEvent, RunnerEventSink, UserInputSubmission};
use crate::agent::Agent;
use crate::llm::ChatResult;
use anyhow::Result;

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
        let result = self
            .agent
            .chat_stream_with_images(
                &input.input,
                input.image_urls.clone(),
                input.turn_id.clone(),
                |event| sink.on_runner_event(RunnerEvent::Agent(event)),
            )
            .await?;
        sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
        Ok(result)
    }
}
