use super::*;
use crate::llm::ChatStreamChunk;
use crate::llm::ChatStreamKind;
use crate::runner::AutomaticInputEvent;
use crate::runner::AutomaticInputKind;
use serde_json::json;

/// 【会话总线】【重连边界】安装确认前的排队终态可回放，确认后的新事件进入订阅。
/// 参数: 无；返回无
#[tokio::test]
async fn ready_subscription_covers_queued_and_future_events() {
    let (handle, _temp) = bus();
    for _ in 0..300 {
        handle
            .emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({}),
            ))
            .unwrap();
    }
    handle
        .emit(WebEvent::new(
            "run",
            "workspace",
            "session",
            "run.completed",
            json!({}),
        ))
        .unwrap();
    // 1. 当前线程尚未让出执行权，所有事件仍排在 AttachReady 前
    let mut subscription = handle.attach_ready().await.unwrap();
    let replay = handle.replay(0);
    assert_eq!(replay.len(), 301);
    assert_eq!(replay.last().unwrap().kind, "run.completed");
    // 2. 安装确认后的事件进入新订阅
    handle
        .emit(WebEvent::new(
            "next",
            "workspace",
            "session",
            "run.started",
            json!({}),
        ))
        .unwrap();
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        subscription.events.recv(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(event.run_id, "next");
    assert_eq!(event.sequence, 302);
}

/// 创建测试用会话事件总线。
fn bus() -> (ActorHandle, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let handle = SessionActor::spawn(temp.path().join("session.jsonl"), "workspace", "session");
    (handle, temp)
}

/// 读取观察者通道中当前可用的全部事件。
fn drain(subscription: &mut SessionSubscription) -> Vec<WebEvent> {
    let mut events = Vec::new();
    while let Ok(event) = subscription.events.try_recv() {
        events.push(event);
    }
    events
}

/// 等待执行者把已入队的命令处理完。
async fn settle(handle: &ActorHandle, expected: usize) {
    let journal = handle.journal();
    for _ in 0..200 {
        if journal.events_after(0).len() >= expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

/// 验证事件会落盘一次并扇出到全部观察者。
#[tokio::test]
async fn fans_out_published_events_to_every_watcher() {
    let (handle, _temp) = bus();
    let mut first = handle.attach().unwrap();
    let mut second = handle.attach().unwrap();

    handle
        .publish(RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
            AutomaticInputKind::ExternalCompletion,
            "后台任务已完成".to_string(),
        )))
        .unwrap();
    settle(&handle, 2).await;

    let first_events = drain(&mut first);
    let second_events = drain(&mut second);
    assert_eq!(first_events.len(), second_events.len());
    assert!(first_events
        .iter()
        .any(|event| event.kind == "message.automatic.input"));
    assert_eq!(handle.journal().events_after(0).len(), first_events.len());
    assert_eq!(first.dropped_events(), 0);
}

/// 验证新一轮会重置组装器状态，避免上一轮状态渗入下一轮。
#[tokio::test]
async fn begins_a_new_run_with_reset_boundary_state() {
    let (handle, _temp) = bus();
    handle
        .publish(RunnerEvent::Agent(crate::agent::AgentEvent::Chunk(
            ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text: "第一轮".to_string(),
            },
        )))
        .unwrap();
    handle.begin_run("run-2", "第二轮输入", &[]).unwrap();
    handle.publish(RunnerEvent::Started).unwrap();
    handle
        .publish(RunnerEvent::Agent(crate::agent::AgentEvent::Chunk(
            ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text: "第二轮".to_string(),
            },
        )))
        .unwrap();
    settle(&handle, 5).await;

    let events = handle.journal().events_after(0);
    let statuses = events
        .iter()
        .filter(|event| event.kind == "status.changed")
        .map(|event| event.payload["status"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    // 轮次边界重置后，第二轮会重新发出 waiting_response / working
    assert_eq!(statuses, ["working", "waiting_response", "working"]);
    let started = events
        .iter()
        .find(|event| event.kind == "run.started")
        .unwrap();
    assert_eq!(started.run_id, "run-2");
    assert_eq!(started.payload["input"], "第二轮输入");
}

/// 验证观察者跟不上时发布不会被阻塞，而是留下 lagged 标记供前端按序号补发。
#[tokio::test]
async fn marks_watcher_lagged_instead_of_blocking() {
    let (handle, _temp) = bus();
    let mut subscription = handle.attach().unwrap();
    let total = WATCHER_CAPACITY * 2;
    for sequence in 0..total {
        handle
            .emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": sequence.to_string() }),
            ))
            .unwrap();
    }
    settle(&handle, total).await;

    // 1. 慢消费者被摘除前仍收到缓冲区内的事件
    let delivered = drain(&mut subscription);
    assert!(delivered.len() <= WATCHER_CAPACITY);
    // 2. 摘除后留下丢弃计数，前端可据此按最后收到的序号重连补发
    assert!(subscription.dropped_events() > 0);
    // 3. 落盘不受慢消费者影响，重连后能补齐空洞
    assert!(handle.journal().events_after(0).len() >= WATCHER_CAPACITY);
    // 4. 摘除后接收端关闭，SSE 流结束并触发客户端重连
    handle
        .emit(WebEvent::new(
            "run",
            "workspace",
            "session",
            "run.completed",
            json!({}),
        ))
        .unwrap();
    settle(&handle, total + 1).await;
    assert!(drain(&mut subscription).is_empty());
    assert!(subscription.events.recv().await.is_none());
}

/// 远程观察者：事件被编码成带序号的帧，且不阻塞扇出。
#[tokio::test]
async fn fans_out_to_remote_watchers_as_numbered_frames() {
    let (handle, _temp) = bus();
    let (mut frames, dropped) = handle.attach_remote(16).unwrap();

    handle.begin_run("run-1", "你好", &[]).unwrap();
    handle
        .emit(WebEvent::new(
            "run-1",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": "hi" }),
        ))
        .unwrap();
    settle(&handle, 1).await;

    let frame = frames.recv().await.expect("远程观察者应当收到帧");
    assert_eq!(frame.kind, KIND_EVT_MIRROR);
    assert_eq!(frame.sequence, Some(1));
    assert_eq!(frame.payload["type"], "message.content.delta");
    assert_eq!(frame.payload["payload"]["text"], "hi");
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
}

/// 远程观察者跟不上时同样被摘除并留下 lagged 标记，扇出不被拖慢。
#[tokio::test]
async fn marks_slow_remote_watcher_lagged_instead_of_blocking() {
    let (handle, _temp) = bus();
    let capacity = 4;
    // 1. 全程不读接收端，模拟卡住的 socket
    let (frames, dropped) = handle.attach_remote(capacity).unwrap();
    let total = capacity * 4;
    for sequence in 0..total {
        handle
            .emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": sequence.to_string() }),
            ))
            .unwrap();
    }
    settle(&handle, total).await;

    // 2. 摘除后留下丢弃计数，且接收端关闭让连接任务结束
    assert!(dropped.load(Ordering::Relaxed) > 0);
    drop(frames);
    // 3. 落盘不受影响：远程观察者重连后按序号补发即可补齐空洞
    assert_eq!(handle.journal().events_after(0).len(), total);
}

/// 新连接按序号补发历史：只发 `after` 之后的事件，序号严格递增。
#[tokio::test]
async fn replays_backlog_from_requested_sequence() {
    let (handle, _temp) = bus();
    for sequence in 0..6u64 {
        handle
            .emit(WebEvent::new(
                "run",
                "workspace",
                "session",
                "message.content.delta",
                json!({ "text": sequence.to_string() }),
            ))
            .unwrap();
    }
    settle(&handle, 6).await;

    // 模拟一个已经收到前 4 条事件的观察者重连
    let backlog = handle.replay(4);
    let sequences = backlog
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, vec![5, 6]);
    assert!(handle.replay(6).is_empty());
}

/// 上行事件（观察者 → 持有者）不再经过组装器，直接落盘并扇出。
#[tokio::test]
async fn mirrors_remote_events_without_reassembling() {
    let (handle, _temp) = bus();
    let mut subscription = handle.attach().unwrap();

    handle
        .mirror(WebEvent::new(
            "run-remote",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": "来自另一个前端" }),
        ))
        .unwrap();
    settle(&handle, 1).await;

    let events = drain(&mut subscription);
    assert_eq!(events.len(), 1, "上行事件不应被组装器拆成多条");
    assert_eq!(events[0].run_id, "run-remote");
    assert_eq!(events[0].payload["text"], "来自另一个前端");
}
