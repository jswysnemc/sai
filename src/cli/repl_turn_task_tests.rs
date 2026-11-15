use super::*;
use crate::config::ProviderConfig;
use crate::runner::{RunnerSubmission, SubmissionSource, UserInputSubmission};
use std::sync::atomic::Ordering;
use tokio::io::AsyncReadExt;

/// 创建阻塞在本地模型请求的真实任务；参数无，返回临时存储、任务与请求到达通知。
async fn pending_turn() -> (
    tempfile::TempDir,
    ReplTurnTask,
    tokio::sync::oneshot::Receiver<()>,
    std::sync::mpsc::Receiver<(String, crate::runner::RunnerEvent)>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 8192];
        let _ = socket.read(&mut request).await.unwrap();
        let _ = sent.send(());
        // 仅保持连接，客户端中断后 read 返回，无需遗留测试任务。
        while socket.read(&mut request).await.unwrap_or(0) != 0 {}
    });
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let mut provider = ProviderConfig::default_openai();
    provider.base_url = format!("http://{address}/v1");
    provider.api_key = Some("test-key".into());
    let mut config = AppConfig {
        active_provider: provider.id.clone(),
        providers: vec![provider],
        load_instruction_files: false,
        ..AppConfig::default()
    };
    config.memory.enabled = false;
    config.skills.enabled = false;
    config.session.auto_title_enabled = false;
    config.retry.max_attempts = 1;
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut agent = Agent::new(
        config.clone(),
        &paths,
        state,
        client,
        crate::tools::ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    agent.prepare_for_turn().unwrap();
    let runtime = ReplRuntime::new(
        1000,
        render::transcript::TranscriptRenderOptions {
            reasoning_mode: render::ReasoningDisplayMode::Summary,
            tool_call_mode: render::ToolCallDisplayMode::Summary,
        },
    );
    let (output, receiver) = std::sync::mpsc::channel();
    let task = ReplTurnTask::start(
        &paths,
        &config,
        agent,
        RunnerSubmission::user_input(
            SubmissionSource::Repl,
            UserInputSubmission::new("等待测试", AgentMode::Yolo),
        ),
        runtime.inter_message_source(),
        output,
    )
    .unwrap();
    (temp, task, received, receiver)
}

/// 验证轮次脱离前台仍被独立调度，取消后交回原会话 Agent；参数无，返回无。
#[tokio::test]
async fn detached_turn_keeps_running_and_returns_original_agent() {
    let (_temp, mut task, received, _output) = pending_turn().await;
    tokio::time::timeout(Duration::from_secs(20), received)
        .await
        .unwrap()
        .unwrap();
    let session = task.session_id.clone();
    assert!(!task.handle.is_finished());
    assert!(!task.cancel_flag.load(Ordering::SeqCst));
    // 接收器没有前台消费也不会阻塞 runner，且切换不设置停止标志。
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!task.handle.is_finished());
    task.interrupt();
    let (agent, result, interrupted) = tokio::time::timeout(Duration::from_secs(5), task.handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(agent.session_id(), session);
    assert!(interrupted);
    assert!(result.is_ok());
}

/// 验证接回同一轮次共享原取消和模式句柄；参数无，返回无。
#[tokio::test]
async fn task_handles_are_stable_while_foreground_moves() {
    let (_temp, mut task, received, _output) = pending_turn().await;
    tokio::time::timeout(Duration::from_secs(20), received)
        .await
        .unwrap()
        .unwrap();
    let mode = task.live_mode.clone();
    let cancel = task.cancel_flag.clone();
    let mut parked = std::collections::HashMap::new();
    let id = task.session_id.clone();
    parked.insert(id.clone(), task);
    task = parked.remove(&id).unwrap();
    assert!(Arc::ptr_eq(&mode, &task.live_mode));
    assert!(Arc::ptr_eq(&cancel, &task.cancel_flag));
    task.interrupt();
    let _ = task.handle.await.unwrap();
}
