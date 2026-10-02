use super::*;
use axum::response::IntoResponse;

/// 【会话事件】【快照交付】历史越界发送一次替换快照，随后仅发送新增量；无参数或返回值。
#[tokio::test]
async fn subscription_resets_expired_history_before_live_events() {
    let root = tempfile::tempdir().unwrap();
    let bus = crate::runner::SessionActor::spawn(root.path().join("events.jsonl"), "workspace", "session");
    let journal = bus.journal();
    journal.publish(WebEvent::new("run", "workspace", "session", "run.started", json!({"input":"question"})));
    for _ in 0..3000 {
        journal.publish(WebEvent::new("run", "workspace", "session", "message.content.delta", json!({"text":"x"})));
    }
    let stream = session_events_stream(bus.clone(), 0).await.unwrap().take(2);
    bus.emit(WebEvent::new("run", "workspace", "session", "message.content.delta", json!({"text":"new"}))).unwrap();
    let response = Sse::new(stream).into_response();
    let bytes = tokio::time::timeout(Duration::from_secs(5), axum::body::to_bytes(response.into_body(), 32 * 1024)).await.unwrap().unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(body.matches("event: stream.reset").count(), 1);
    let payloads = body.lines().filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).unwrap()).collect::<Vec<_>>();
    assert_eq!(payloads[0]["through_sequence"], 3001);
    assert_eq!(payloads[0]["events"][1]["payload"]["text"], "x".repeat(3000));
    assert_eq!(payloads[1]["sequence"], 3002);
    assert_eq!(payloads[1]["payload"]["text"], "new");
    assert_eq!(payloads[1]["replayed"], false);
}

/// 【会话事件】【订阅边界】安装订阅前排队的结束事件必须补发一次；无参数或返回值。
#[tokio::test]
async fn subscription_replays_terminal_event_queued_before_attach() {
    let root = tempfile::tempdir().unwrap();
    let bus = crate::runner::SessionActor::spawn(root.path().join("events.jsonl"), "workspace", "session");
    bus.emit(WebEvent::new("run", "workspace", "session", "run.completed", json!({"content":"complete"}))).unwrap();
    // 1. 当前线程还没有让出执行权，结束事件仍排在订阅命令前
    let stream = session_events_stream(bus, 0).await.unwrap().take(1);
    let response = Sse::new(stream).into_response();
    let bytes = tokio::time::timeout(Duration::from_millis(500), axum::body::to_bytes(response.into_body(), 16 * 1024)).await
        .expect("queued terminal event was lost during subscription").unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(body.matches("event: run.completed").count(), 1);
}

/// 【通知交付测试】【真实事件流】历史与实时事件使用同一日志序号，只在 SSE 封套中区分补发。
#[tokio::test]
async fn notification_delivery_marks_replay_without_mutating_the_journal() {
    let root = tempfile::tempdir().unwrap();
    let bus = crate::runner::SessionActor::spawn(
        root.path().join("events.jsonl"),
        "workspace",
        "session",
    );
    let mut initial = bus.attach().unwrap();
    bus.emit(WebEvent::new(
        "old",
        "workspace",
        "session",
        "run.completed",
        json!({"content":"old reply"}),
    ))
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), initial.events.recv())
        .await
        .unwrap()
        .unwrap();
    drop(initial);
    let stream = session_events_stream(bus.clone(), 0).await.unwrap().take(2);
    bus.emit(WebEvent::new(
        "new",
        "workspace",
        "session",
        "run.failed",
        json!({"message":"failed reply"}),
    ))
    .unwrap();
    let response = Sse::new(stream).into_response();
    let bytes = tokio::time::timeout(
        Duration::from_secs(5),
        axum::body::to_bytes(response.into_body(), 16 * 1024),
    )
    .await
    .unwrap()
    .unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    let payloads = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(payloads.len(), 2, "{body}");
    assert_eq!(payloads[0]["sequence"], 1);
    assert_eq!(payloads[0]["run_id"], "old");
    assert_eq!(payloads[0]["payload"]["content"], "old reply");
    assert_eq!(payloads[0]["replayed"], true);
    assert_eq!(payloads[1]["sequence"], 2);
    assert_eq!(payloads[1]["replayed"], false);
    assert_eq!(payloads[1]["payload"]["message"], "failed reply");
    let journal = bus.journal().events_after(0);
    assert_eq!(journal.len(), 2);
    assert!(serde_json::to_value(journal)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .all(|event| event.get("replayed").is_none()));
}
