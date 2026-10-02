use super::*;
use crate::web::runs::EventJournal;
use serde_json::json;

/// 【会话恢复】【测试事件】参数为序号和类型，返回同一会话的事件。
fn event(sequence: u64, kind: &str) -> WebEvent {
    let mut event = WebEvent::new(
        "run",
        "workspace",
        "session",
        kind,
        json!({"text":"x", "input":"question"}),
    );
    event.sequence = sequence;
    event
}

/// 【会话恢复】【合并边界】大量正文合并后保留工具与待处理请求；无参数或返回值。
#[test]
fn snapshot_preserves_text_tools_and_requests() {
    let journal = EventJournal::new();
    journal.publish(event(0, "run.started"));
    for _ in 0..3000 {
        journal.publish(event(0, "message.content.delta"));
    }
    journal.publish(event(0, "tool.call.started"));
    journal.publish(event(0, "permission.requested"));
    journal.publish(event(0, "ssh.secret.requested"));
    journal.publish(event(0, "message.content.delta"));
    let replay = journal.replay_after(0);
    assert!(replay.events.is_empty());
    let reset = replay.reset.unwrap();
    assert!(reset.incomplete_run_ids.is_empty());
    assert_eq!(reset.events.len(), 6);
    assert_eq!(reset.events[0].kind, "run.started");
    assert_eq!(reset.events[1].payload["text"], "x".repeat(3000));
    assert_eq!(reset.events[2].kind, "tool.call.started");
    assert_eq!(reset.events[3].kind, "permission.requested");
    assert_eq!(reset.events[4].kind, "ssh.secret.requested");
    assert_eq!(reset.events[5].payload["text"], "x");
    let delta = journal.replay_after(3004);
    assert!(delta.reset.is_none());
    assert_eq!(delta.events.len(), 1);
    assert_eq!(delta.events[0].sequence, 3005);
}

/// 【会话恢复】【持久化】压缩前后重启都能恢复完整活动文本；无参数或返回值。
#[test]
fn restart_restores_checkpoint_and_uncompacted_tail() {
    for count in [3000, 6000] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("events.jsonl");
        let journal = EventJournal::persistent(path.clone());
        journal.publish(event(0, "run.started"));
        for _ in 0..count {
            journal.publish(event(0, "message.content.delta"));
        }
        drop(journal);
        let reopened = EventJournal::persistent(path.clone());
        let reset = reopened.replay_after(0).reset.unwrap();
        assert!(reset.incomplete_run_ids.is_empty());
        assert_eq!(reset.events[1].payload["text"], "x".repeat(count));
        assert_eq!(reset.through_sequence, count as u64 + 1);
        assert_eq!(
            reopened.publish(event(0, "message.content.delta")).sequence,
            count as u64 + 2
        );
        drop(reopened);
        let reopened = EventJournal::persistent(path);
        assert_eq!(
            reopened.replay_after(0).reset.unwrap().events[1].payload["text"],
            "x".repeat(count + 1)
        );
    }
}

/// 【会话恢复】【水位回退】高于服务端的水位必须触发清空恢复；无参数或返回值。
#[test]
fn stale_client_sequence_resets_even_an_empty_journal() {
    let journal = EventJournal::new();
    let reset = journal.replay_after(100).reset.unwrap();
    assert_eq!(reset.through_sequence, 0);
    assert!(reset.events.is_empty());
    journal.publish(event(0, "run.started"));
    journal.publish(event(0, "run.completed"));
    assert!(journal.replay_after(100).reset.unwrap().events.is_empty());
}

/// 【会话恢复】【容量提示】超限明确标记，结束事件释放恢复状态；无参数或返回值。
#[test]
fn snapshot_marks_capacity_and_missing_entry() {
    let mut snapshot = ReplaySnapshot::default();
    snapshot.observe_bounded(&event(1, "run.started"), 500);
    let mut large = event(2, "message.content.delta");
    large.payload = json!({"text":"x".repeat(1000)});
    snapshot.observe_bounded(&large, 500);
    assert_eq!(snapshot.reset().incomplete_run_ids, vec!["run"]);
    assert!(snapshot.bytes <= 500);
    snapshot.observe(&event(3, "run.completed"));
    assert!(snapshot.reset().events.is_empty());
    assert!(snapshot.reset().incomplete_run_ids.is_empty());
    snapshot.observe(&event(4, "message.content.delta"));
    assert_eq!(snapshot.reset().incomplete_run_ids, vec!["run"]);
}
