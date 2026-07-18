use crate::agent::AgentEvent;
use crate::llm::ChatResult;
use crate::state::SessionSnapshot;
use anyhow::Result;

/// runner 事件。
#[derive(Debug, Clone)]
pub(crate) enum RunnerEvent {
    Started,
    /// Goal 仍未完成，正在等待后台命令或子 Agent 完成后自动续轮
    WaitingExternal,
    Agent(AgentEvent),
    Interrupted,
    Completed(ChatResult),
    Failed(String),
    LoadedToolsChanged(Vec<String>),
    FinalSummary(SessionSnapshot),
}

/// runner 输出汇总。
#[derive(Debug, Clone, Default)]
pub(crate) struct RunnerOutput {
    pub(crate) events: Vec<RunnerEvent>,
    pub(crate) completion: Option<ChatResult>,
}

impl RunnerOutput {
    /// 追加 runner 事件。
    ///
    /// 参数:
    /// - `event`: runner 事件
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_event(&mut self, event: RunnerEvent) {
        if let RunnerEvent::Completed(result) = &event {
            self.completion = Some(result.clone());
        }
        self.events.push(event);
    }
}

/// runner 事件接收器。
pub(crate) trait RunnerEventSink {
    /// 处理 runner 事件。
    ///
    /// 参数:
    /// - `event`: runner 事件
    ///
    /// 返回:
    /// - 处理是否成功
    fn on_runner_event(&mut self, event: RunnerEvent) -> Result<()>;
}

impl<F> RunnerEventSink for F
where
    F: FnMut(RunnerEvent) -> Result<()>,
{
    /// 处理 runner 事件闭包。
    ///
    /// 参数:
    /// - `event`: runner 事件
    ///
    /// 返回:
    /// - 处理是否成功
    fn on_runner_event(&mut self, event: RunnerEvent) -> Result<()> {
        self(event)
    }
}
