use super::*;
use axum::response::IntoResponse;

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
    let stream = session_events_stream(bus.clone(), 0).unwrap().take(2);
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
