use crate::agent::{Agent, ExternalEventWake};
use anyhow::Result;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// 管理 TUI 会话外部完成事件的一次性监听任务。
pub(super) struct ReplExternalEvents {
    receiver: Option<mpsc::UnboundedReceiver<Result<Option<ExternalEventWake>>>>,
    task: Option<JoinHandle<()>>,
    paused_session: Option<std::path::PathBuf>,
}

impl ReplExternalEvents {
    /// 创建尚未启动监听的外部事件管理器。
    ///
    /// 返回:
    /// - 空的外部事件管理器
    pub(super) fn new() -> Self {
        Self {
            receiver: None,
            task: None,
            paused_session: None,
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
        if self.paused_session.as_deref() == Some(agent.state().state_dir()) {
            return;
        }
        self.paused_session = None;
        let monitor = agent.external_event_monitor();
        // 1. 【自动续聊】【监听隔离】每次监听独占通道，旧任务迟到结果无法进入新轮次
        let (sender, receiver) = mpsc::unbounded_channel();
        self.receiver = Some(receiver);
        self.task = Some(tokio::spawn(async move {
            let _ = sender.send(monitor.wait_for_wake().await);
        }));
    }

    /// 非阻塞读取已经就绪的自动唤醒事件。
    ///
    /// 返回:
    /// - 尚未就绪时返回空；监听失败时保留错误
    pub(super) fn take_ready(&mut self) -> Option<Result<ExternalEventWake>> {
        match self.receiver.as_mut()?.try_recv() {
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
        self.receiver = None;
    }

    /// 【自动续聊】【失败等待】暂停当前会话的自动唤醒，保留未确认通知。
    ///
    /// 参数:
    /// - `session_dir`: 包含工作区作用域的会话状态目录
    ///
    /// 返回:
    /// - 无；用户再次提交或切换会话后恢复监听
    pub(super) fn pause(&mut self, session_dir: &std::path::Path) {
        self.cancel();
        self.paused_session = Some(session_dir.to_path_buf());
    }

    /// 【自动续聊】【失败等待】用户主动提交后允许再次自动唤醒。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无，不提前确认失败轮次的通知
    pub(super) fn resume(&mut self) {
        self.paused_session = None;
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

    /// 【自动续聊】【监听测试】创建无需网络请求的隔离 Agent。
    ///
    /// 参数: paths 为临时目录集合
    /// 返回: 用于检验监听生命周期的 Agent
    fn test_agent(paths: &crate::paths::SaiPaths) -> Agent {
        let mut config = crate::config::AppConfig::default();
        config.memory.enabled = false;
        config.skills.enabled = false;
        config.load_instruction_files = false;
        let state = crate::state::StateStore::new(paths).unwrap();
        state.init_files().unwrap();
        let client = crate::llm::OpenAiCompatibleClient::from_config(&config, paths).unwrap();
        Agent::new(
            config,
            paths,
            state,
            client,
            crate::tools::ToolRegistry::new(),
            crate::agent::AgentMode::Yolo,
        )
        .unwrap()
    }

    /// 【自动续聊】【失败等待】验证失败暂停不会自动重试，用户提交和会话切换可恢复。
    ///
    /// 参数: 无
    /// 返回: 无；暂停后自行唤醒或无法恢复时断言失败
    #[tokio::test]
    async fn automatic_failure_waits_for_user_or_session_change() {
        let temp = tempfile::tempdir().unwrap();
        let first_paths = crate::paths::SaiPaths::for_tests(&temp.path().join("first"));
        let second_paths = crate::paths::SaiPaths::for_tests(&temp.path().join("second"));
        let first = test_agent(&first_paths);
        let second = test_agent(&second_paths);
        let mut events = ReplExternalEvents::new();
        events.arm(&first);
        assert!(events.is_armed());
        events.pause(first.state().state_dir());
        events.arm(&first);
        assert!(!events.is_armed());

        events.resume();
        events.arm(&first);
        assert!(events.is_armed());
        events.pause(first.state().state_dir());
        events.arm(&second);
        assert!(events.is_armed());
    }

    /// 【自动续聊】【监听取消】验证已排队的旧通知随监听一起失效。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；取消后仍能读取完成事件时断言失败
    #[test]
    fn cancelled_listener_discards_queued_completion() {
        let mut events = ReplExternalEvents::new();
        let (sender, receiver) = mpsc::unbounded_channel();
        events.receiver = Some(receiver);
        sender
            .send(Ok(Some(ExternalEventWake::Completion(
                crate::agent::ExternalEventBatch::for_test("completed", "done"),
            ))))
            .unwrap();
        events.cancel();
        assert!(events.take_ready().is_none(), "旧监听通知不能触发新轮次");
        assert!(sender.send(Ok(None)).is_err(), "旧任务的迟到结果必须失效");
    }

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
