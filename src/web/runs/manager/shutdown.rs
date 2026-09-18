use super::*;
use std::sync::atomic::Ordering;

impl RunManager {
    /// 【Web】【日志配置】启用此服务的会话控制台日志；无参数，返回管理器
    pub(crate) fn with_console_logging(mut self) -> Self {
        self.console_logging = true;
        self
    }

    /// 【Web】【关闭运行】冻结调度，终止本进程活动轮次，保留队列供下次启动恢复。
    /// 参数: 无
    /// 返回: 检查点保存结果；所有活动任务都会先收到取消
    pub(crate) async fn shutdown(&self) -> Result<()> {
        self.shutting_down.store(true, Ordering::SeqCst);
        let _scheduling = self.scheduling.lock().await;
        let runs = self
            .active
            .lock()
            .await
            .drain()
            .map(|(_, run)| run)
            .collect::<Vec<_>>();
        for run in &runs {
            run.cancel_requested.store(true, Ordering::SeqCst);
            run.handle.abort();
        }
        // 1. 【Web】【退出保存】并行等待取消任务释放轮次守卫，避免等待时间随会话数累加
        let results = futures_util::future::join_all(runs.into_iter().map(|run| async move {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), run.handle).await;
            // 2. 【Web】【退出事件】等待终态落盘，再完成检查点；中途退出仍可由启动恢复补齐
            self.publish_shutdown_interruption(&run.info).await?;
            self.checkpoints
                .update_interruption(&run.info.run_id, false, None)
        }))
        .await;
        results.into_iter().collect::<Result<Vec<_>>>().map(|_| ())
    }

    /// 【Web】【退出事件】发布中断事件并等待订阅确认，保证服务返回前已完成落盘。
    /// 参数: info 为被取消的活动运行；返回事件发布及确认结果
    async fn publish_shutdown_interruption(&self, info: &ActiveRunInfo) -> Result<()> {
        let bus = self.session_bus(&info.workspace_id, &info.session_id).await;
        let mut subscription = bus
            .attach_ready()
            .await
            .ok_or_else(|| anyhow::anyhow!("session event bus is closed"))?;
        bus.emit(WebEvent::new(
            &info.run_id,
            &info.workspace_id,
            &info.session_id,
            "run.interrupted",
            json!({
                "shutdown": true,
                "discard_user_turn": false,
                "restore_input": null,
                "detail": "Sai stopped this run while shutting down.",
            }),
        ))?;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while let Some(event) = subscription.events.recv().await {
                if event.run_id == info.run_id
                    && event.kind == "run.interrupted"
                    && event.payload["shutdown"] == true
                {
                    return Ok(());
                }
            }
            Err(anyhow::anyhow!(
                "session event bus closed before shutdown confirmation"
            ))
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out saving shutdown event"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【Web】【关闭验证】活动任务取消、队列保留且禁止自动重启；无参数，无返回值
    #[tokio::test]
    async fn shutdown_cancels_owned_runs_and_preserves_queue() {
        let root = tempfile::tempdir().unwrap();
        let manager = RunManager::new(&SaiPaths::for_tests(root.path())).unwrap();
        let checkpoint = super::super::tests::test_checkpoint(
            root.path(),
            "running",
            RunCheckpointStatus::Running,
        );
        manager.checkpoints.upsert(checkpoint.clone()).unwrap();
        let key = session_key(&checkpoint.info.workspace_id, &checkpoint.info.session_id);
        let bus = manager
            .session_bus(&checkpoint.info.workspace_id, &checkpoint.info.session_id)
            .await;
        let mut observer = bus.attach().unwrap();
        let handle = tokio::spawn(std::future::pending());
        let aborted = handle.abort_handle();
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        manager.active.lock().await.insert(
            key.clone(),
            ActiveRun {
                info: checkpoint.info,
                handle,
                cancel_requested: cancelled.clone(),
            },
        );
        let mut queued = super::super::tests::test_checkpoint(
            root.path(),
            "queued",
            RunCheckpointStatus::Queued,
        );
        queued.info.session_id = "session-running".into();
        manager.checkpoints.upsert(queued.clone()).unwrap();
        manager.queued.lock().await.insert(
            key.clone(),
            VecDeque::from([QueuedRun {
                info: queued.info,
                workspace: queued.workspace,
                request: queued.request,
            }]),
        );
        manager.shutdown().await.unwrap();
        manager.launch_next(&key).await;
        assert!(aborted.is_finished());
        assert!(cancelled.load(Ordering::SeqCst));
        assert!(manager.active.lock().await.is_empty());
        assert_eq!(manager.queued.lock().await[&key].len(), 1);
        assert_eq!(
            manager.checkpoints.get("running").unwrap().status,
            RunCheckpointStatus::Interrupted
        );
        assert_eq!(
            manager.checkpoints.get("queued").unwrap().status,
            RunCheckpointStatus::Queued
        );
        let event = observer.events.try_recv().unwrap();
        assert_eq!(event.kind, "run.interrupted");
        assert_eq!(event.payload["shutdown"], true);
        let events = EventJournal::persistent(manager.session_event_path(&key)).events_after(0);
        assert!(events
            .iter()
            .any(|event| { event.run_id == "running" && event.kind == "run.interrupted" }));
    }
}
