use super::*;
use crate::agent::{Agent, InterMessageSource, LiveAgentMode};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
use tokio::sync::oneshot;

/// 独占一轮 Agent 的任务及其会话隔离的事件接收端。
pub(in crate::cli) struct ReplTurnTask {
    pub events: mpsc::Receiver<crate::runner::RunnerEvent>,
    pub handle: tokio::task::JoinHandle<(Agent, Result<()>, bool)>,
    pub live_mode: Arc<LiveAgentMode>,
    pub cancel_flag: Arc<AtomicBool>,
    pub session_id: String,
    stop: Option<oneshot::Sender<()>>,
    pub interrupted_batch: Option<crate::agent::ExternalEventBatch>,
}

impl ReplTurnTask {
    /// 启动拥有 Agent 的后台轮次；参数为路径、配置、Agent、提交及本会话队列来源；返回任务槽。
    pub(in crate::cli) fn start(
        paths: &SaiPaths,
        config: &AppConfig,
        mut agent: Agent,
        submission: crate::runner::RunnerSubmission,
        source: Arc<dyn InterMessageSource>,
        output: mpsc::Sender<(String, crate::runner::RunnerEvent)>,
    ) -> Result<Self> {
        let directory = crate::runtime_cwd::current_dir()?;
        let paths = paths.clone();
        let config = config.clone();
        let live_mode = agent.live_mode_handle();
        let cancel_flag = agent.cancel_handle();
        let session_id = agent.session_id().to_owned();
        let (sender, events) = mpsc::channel();
        let event_session = session_id.clone();
        let (stop, stopped) = oneshot::channel();
        let handle = tokio::spawn(crate::runtime_cwd::scope(directory, async move {
            let runner = crate::runner::SessionRunner::new(&paths)
                .with_config(config.clone())
                .with_inter_message_source(source);
            let mut sink = move |event| {
                if super::repl_turn::turn_event_needs_foreground(&event)
                    || matches!(
                        event,
                        crate::runner::RunnerEvent::Agent(crate::agent::AgentEvent::ExternalOutput)
                    )
                {
                    sender
                        .send(event)
                        .map_err(|_| anyhow::anyhow!("session interaction channel is closed"))
                } else {
                    output
                        .send((event_session.clone(), event))
                        .map_err(|_| anyhow::anyhow!("session output channel is closed"))
                }
            };
            let (result, interrupted) = {
                let run = runner.run_submission_with_agent(submission, &mut agent, &mut sink);
                tokio::pin!(run);
                tokio::select! {
                    result = &mut run => (result.map(|_| ()), false),
                    _ = stopped => (Ok(()), true),
                }
            };
            let status = if interrupted {
                sai_plugin_runtime::ReplyStatus::Interrupted
            } else if result.is_err() {
                sai_plugin_runtime::ReplyStatus::Failed
            } else {
                sai_plugin_runtime::ReplyStatus::Completed
            };
            crate::notifications::reply_ended(&paths, &config, status).await;
            (agent, result, interrupted)
        }));
        Ok(Self {
            events,
            handle,
            live_mode,
            cancel_flag,
            session_id,
            stop: Some(stop),
            interrupted_batch: None,
        })
    }

    /// 请求中断本轮但保留 Agent；参数无，返回无，切换前台绝不调用此方法。
    pub(in crate::cli) fn interrupt(&mut self) {
        self.cancel_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

#[cfg(test)]
#[path = "repl_turn_task_tests.rs"]
mod tests;
