use super::*;

/// 执行 Agent 并把 RunnerEvent 写入会话事件总线。
async fn run_agent(
    paths: SaiPaths,
    request: StartRunRequest,
    info: ActiveRunInfo,
    bus: ActorHandle,
    inter_message_source: Arc<dyn InterMessageSource>,
    cancel_requested: Arc<std::sync::atomic::AtomicBool>,
) -> RunCheckpointStatus {
    let mode = match AgentMode::parse(request.mode.as_deref()) {
        Ok(mode) => mode,
        Err(error) => {
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
            ));
            return RunCheckpointStatus::Failed;
        }
    };
    let submission = match request.kind {
        RunKind::Conversation => {
            let mut input = UserInputSubmission::new(request.input, mode);
            input = input.with_image_urls(request.image_url.into_iter().chain(request.image_urls));
            input = input.with_turn_id(info.run_id.clone());
            RunnerSubmission::user_input(SubmissionSource::Web, input)
        }
        RunKind::Compaction => RunnerSubmission::control(
            SubmissionSource::Web,
            mode,
            ControlSubmission::new(crate::control_commands::ControlCommand::Compact),
        ),
        RunKind::GoalContinuation => RunnerSubmission::user_input(
            SubmissionSource::Web,
            UserInputSubmission::new(String::new(), mode).with_goal_continuation(),
        ),
    }
    .with_session_id(info.session_id.clone())
    .with_final_summary(true);
    // 会话级组装器由事件总线持有，这里只声明轮次边界
    let _ = bus.begin_run(&info.run_id, &info.input, &info.image_urls);
    let mut sink = |event| bus.publish(event);
    let run_config = match resolve_run_config(
        &paths,
        request.agent_id.as_deref(),
        request.provider_id.as_deref(),
        request.model.as_deref(),
        request.thinking_level.as_deref(),
    ) {
        Ok(config) => config,
        Err(error) => {
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
            ));
            return RunCheckpointStatus::Failed;
        }
    };
    let runner = match run_config {
        Some(config) => SessionRunner::new(&paths).with_config(config),
        None => SessionRunner::new(&paths),
    }
    .with_inter_message_source(inter_message_source)
    .with_cancel_flag(cancel_requested);
    if let Err(error) = runner.run_submission(submission, &mut sink).await {
        let _ = bus.emit(WebEvent::new(
            &info.run_id,
            &info.workspace_id,
            &info.session_id,
            "run.failed",
            json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
        ));
        return RunCheckpointStatus::Failed;
    }
    RunCheckpointStatus::Completed
}

impl RunManager {
    /// 启动已经取得会话执行权的运行。
    pub(super) fn spawn_run(
        &self,
        key: String,
        queued: QueuedRun,
        bus: ActorHandle,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let (start_tx, start_rx) = oneshot::channel();
            let manager = self.clone();
            let task_info = queued.info.clone();
            let inter_message_source: Arc<dyn InterMessageSource> = Arc::new(WebMessageQueue::new(
                self.clone(),
                key.clone(),
                task_info.run_id.clone(),
            ));
            let workspace_path = std::path::PathBuf::from(&queued.workspace.path);
            let paths = self.paths.clone();
            let task_key = key.clone();
            // 标志在 spawn 前创建：stop 需要在任务被 abort 之前置位
            let cancel_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let task_cancel = cancel_requested.clone();
            let continuation = queued.clone();
            let handle = tokio::spawn(async move {
                let _ = start_rx.await;
                let terminal = crate::runtime_cwd::scope(
                    workspace_path,
                    run_agent(
                        paths,
                        queued.request,
                        task_info.clone(),
                        bus.clone(),
                        inter_message_source,
                        task_cancel,
                    ),
                )
                .await;
                let _ = manager
                    .checkpoints
                    .update_status(&task_info.run_id, terminal);
                manager.clear_active_if(&task_key).await;
                // 1. 用户队列优先；队列空且目标仍活动时再排续轮
                manager
                    .schedule_goal_continuation(&task_key, &continuation, terminal)
                    .await;
                manager.launch_next(&task_key).await;
            });
            self.active.lock().await.insert(
                key,
                ActiveRun {
                    info: queued.info,
                    handle,
                    cancel_requested,
                },
            );
            let _ = start_tx.send(());
        })
    }
}
