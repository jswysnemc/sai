use crate::agent::{Agent, ExternalEventWake};
use anyhow::Result;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// 管理 TUI 会话外部完成事件的一次性监听任务。
pub(super) struct ReplExternalEvents {
    receiver: mpsc::UnboundedReceiver<Result<Option<ExternalEventWake>>>,
    sender: mpsc::UnboundedSender<Result<Option<ExternalEventWake>>>,
    task: Option<JoinHandle<()>>,
}

impl ReplExternalEvents {
    /// 创建尚未启动监听的外部事件管理器。
    ///
    /// 返回:
    /// - 空的外部事件管理器
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            receiver,
            sender,
            task: None,
        }
    }

    /// 在当前对话轮次释放 Agent 后启动一次监听。
    ///
    /// 参数:
    /// - `agent`: 当前会话 Agent
    ///
    /// 返回:
    /// - 无
    pub(super) fn arm(&mut self, agent: &Agent) {
        self.cancel();
        while self.receiver.try_recv().is_ok() {}
        let monitor = agent.external_event_monitor();
        let sender = self.sender.clone();
        self.task = Some(tokio::spawn(async move {
            let _ = sender.send(monitor.wait_for_wake().await);
        }));
    }

    /// 非阻塞读取已经就绪的自动唤醒事件。
    ///
    /// 返回:
    /// - 尚未就绪时返回空；监听失败时保留错误
    pub(super) fn take_ready(&mut self) -> Option<Result<ExternalEventWake>> {
        match self.receiver.try_recv() {
            Ok(Ok(Some(wake))) => {
                self.task.take();
                Some(Ok(wake))
            }
            Ok(Ok(None)) => {
                self.task.take();
                None
            }
            Ok(Err(error)) => {
                self.task.take();
                Some(Err(error))
            }
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => None,
        }
    }

    /// 判断监听结果是否仍可能唤醒输入循环。
    ///
    /// 返回:
    /// - 监听任务存在且结果尚未消费时返回 `true`
    pub(super) fn is_armed(&self) -> bool {
        self.task.is_some()
    }

    /// 取消旧会话或旧轮次对应的监听任务。
    ///
    /// 返回:
    /// - 无
    pub(super) fn cancel(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for ReplExternalEvents {
    /// 退出 TUI 时终止残留监听任务。
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证未启动监听时不会阻塞输入轮询。
    #[test]
    fn idle_manager_has_no_ready_wake() {
        let mut events = ReplExternalEvents::new();

        assert!(!events.is_armed());
        assert!(events.take_ready().is_none());
    }

    /// 验证任务结束到结果消费之间仍保持输入轮询。
    #[tokio::test]
    async fn finished_task_stays_armed_until_result_is_consumed() {
        let mut events = ReplExternalEvents::new();
        events.task = Some(tokio::spawn(async {}));
        tokio::task::yield_now().await;

        assert!(events.is_armed());
    }
}
