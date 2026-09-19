use super::*;
use std::time::{Duration, Instant};

/// 请求序号、光标或输入任何一项变化时拒绝旧结果
#[test]
fn stale_responses_cannot_replace_current_candidates() {
    let snapshot = Snapshot {
        input: "@src".into(),
        cursor: 4,
        cwd: PathBuf::from("/workspace"),
    };
    let mut completion = MentionCompletion {
        id: 7,
        snapshot: Some(snapshot.clone()),
        pending: true,
        ..Default::default()
    };
    for (id, input, cursor) in [(6, "@src", 4), (7, "read @src", 9), (7, "@src", 3)] {
        assert!(!completion.accept(Response {
            id,
            snapshot: Snapshot {
                input: input.into(),
                cursor,
                cwd: snapshot.cwd.clone()
            },
            items: vec![],
        }));
        assert!(completion.pending());
    }
    assert!(completion.accept(Response {
        id: 7,
        snapshot,
        items: vec![]
    }));
    assert!(!completion.pending());
}

/// 目录查询不阻塞调用者，快速改词只返回最后一次输入对应的候选
#[test]
fn background_scan_returns_latest_query_and_cancels_on_exit() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("alpha.txt"), "").unwrap();
    std::fs::write(root.path().join("beta.txt"), "").unwrap();
    let first = format!("@{}/a", root.path().display());
    let last = format!("@{}/b", root.path().display());
    let mut completion = MentionCompletion::default();
    completion.query(&first, first.chars().count(), &[]);
    let mut items = completion.query(&last, last.chars().count(), &[]);
    let deadline = Instant::now() + Duration::from_secs(3);
    while completion.pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        items = completion.query(&last, last.chars().count(), &[]);
    }
    assert!(!completion.pending());
    assert_eq!(items.len(), 1);
    assert!(items[0].insert.ends_with("beta.txt"));
    completion.query(&first, first.chars().count(), &[]);
    assert!(completion.query("plain input", 11, &[]).is_empty());
    assert!(!completion.pending());
}

/// 取消中的目录扫描不应缓存部分条目，恢复查询后仍能找到完整列表
#[test]
fn interrupted_scan_does_not_poison_directory_cache() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..30 {
        std::fs::write(root.path().join(format!("file-{index:02}")), "").unwrap();
    }
    let mut cache = None;
    let checks = std::cell::Cell::new(0);
    let result = super::super::files::complete(root.path(), "", &mut cache, || {
        checks.set(checks.get() + 1);
        checks.get() > 3
    });
    assert!(result.is_none());
    assert!(cache.is_none());
    let result =
        super::super::files::complete(root.path(), "file-29", &mut cache, || false).unwrap();
    assert_eq!(result[0].label, "file-29");
}
