use super::super::{record::Flag, store::Store};
use super::{fixture, save};
use sai_plugin_runtime::host::ScheduledStatus;

/// 【旧闹钟测试】【损坏与限额】拒绝损坏 JSON、重复标识、过多记录和过大文件。
/// @returns 无
#[test]
fn legacy_storage_rejects_corrupt_duplicate_and_oversized_records() {
    let (_root, paths, record) = fixture();
    let store = Store::open(&paths).unwrap();
    for bytes in [b"not json".to_vec(), vec![b' '; 4 * 1024 * 1024 + 1]] {
        std::fs::write(paths.state_dir.join("alarms.json"), bytes).unwrap();
        assert!(store.records().is_err());
    }
    save(&paths, &[record.clone(), record.clone()]);
    assert!(store.records().is_err());
    let records: Vec<_> = (0..129)
        .map(|index| {
            let mut item = record.clone();
            item.id = format!("alarm-1700000000000-{}", index + 1);
            item
        })
        .collect();
    save(&paths, &records);
    assert!(store.records().is_err());
    save(&paths, &records[..128]);
    assert_eq!(store.records().unwrap().len(), 128);
}

/// 【旧闹钟测试】【记录验证】非法标识、时间、PID 和相对音频路径不能进入兼容层。
/// @returns 无
#[test]
fn legacy_record_validation_rejects_invalid_fields() {
    let (_root, paths, record) = fixture();
    let original = serde_json::to_value(&record).unwrap();
    for (key, invalid) in [
        ("id", serde_json::json!("../../elsewhere")),
        ("id", serde_json::json!("alarm-1-0")),
        ("due_at", serde_json::json!(-1)),
        ("pid", serde_json::json!(0)),
        ("audio_file", serde_json::json!("relative.wav")),
        ("label", serde_json::json!("x".repeat(4097))),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut value = original.clone();
        value[key] = invalid;
        std::fs::write(
            paths.state_dir.join("alarms.json"),
            serde_json::to_vec(&vec![value]).unwrap(),
        )
        .unwrap();
        assert!(Store::open(&paths).unwrap().records().is_err(), "{key}");
    }
}

/// 【旧闹钟测试】【比较转换】记录内容变化后，旧工作入口不能更新新记录状态。
/// @returns 无
#[test]
fn legacy_transitions_reject_changed_record_identity() {
    let (_root, paths, record) = fixture();
    let store = Store::open(&paths).unwrap();
    let mut changed = record.clone();
    changed.due_at -= 1;
    save(&paths, &[changed]);
    assert!(store
        .transition(
            &record,
            Some(Flag::new(&record, ScheduledStatus::Cancelled, None).unwrap()),
            &[ScheduledStatus::Scheduled]
        )
        .is_err());
}

/// 【旧闹钟测试】【独立状态校验】未知版本、错误时间与过大状态不能改变任务状态。
/// @returns 无
#[test]
fn legacy_storage_rejects_invalid_independent_states() {
    let (_root, paths, record) = fixture();
    let store = Store::open(&paths).unwrap();
    let (_, directory) =
        crate::plugins::private::paths::namespace(&paths.state_dir, "plugin-legacy", "alarm", "")
            .unwrap();
    let mut invalid = Flag::new(&record, ScheduledStatus::Completed, None).unwrap();
    invalid.finished_at = None;
    let invalid_records = std::collections::BTreeMap::from([(record.task_id(), invalid)]);
    for bytes in [
        br#"{"version":2,"records":{}}"#.to_vec(),
        serde_json::to_vec(&serde_json::json!({"version":1,"records":invalid_records})).unwrap(),
        vec![b' '; 1024 * 1024 + 1],
    ] {
        std::fs::write(directory.join("states.json"), bytes).unwrap();
        assert!(store.flag(&record).is_err());
    }
}

/// 【旧闹钟测试】【特殊文件隔离】原文件与独立状态都拒绝链接，管道不能阻塞读取。
/// @returns 无
#[cfg(unix)]
#[test]
fn legacy_storage_rejects_symlinks_and_fifos() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let (root, paths, record) = fixture();
    let source = paths.state_dir.join("alarms.json");
    let outside = root.path().join("outside.json");
    std::fs::rename(&source, &outside).unwrap();
    symlink(&outside, &source).unwrap();
    let store = Store::open(&paths).unwrap();
    assert!(store.records().is_err());
    std::fs::remove_file(&source).unwrap();
    let name = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(store.records().is_err());
    std::fs::remove_file(&source).unwrap();
    save(&paths, std::slice::from_ref(&record));
    let (_, directory) =
        crate::plugins::private::paths::namespace(&paths.state_dir, "plugin-legacy", "alarm", "")
            .unwrap();
    symlink(outside, directory.join("states.json")).unwrap();
    assert!(store.flag(&record).is_err());
}

/// 【旧闹钟测试】【执行锁清理】保留正在持有的历史锁，释放后在下一次执行前清理。
/// @returns 无
#[test]
fn legacy_worker_lock_cleanup_keeps_live_leases() {
    let (_root, paths, record) = fixture();
    let store = Store::open(&paths).unwrap();
    let first = store.worker_lock(&record).unwrap();
    assert!(store.worker_lock(&record).is_err());
    let mut next = record.clone();
    next.id = "alarm-1700000000000-322".into();
    save(&paths, std::slice::from_ref(&next));
    let second = store.worker_lock(&next).unwrap();
    let (_, directory) =
        crate::plugins::private::paths::namespace(&paths.state_dir, "plugin-legacy", "alarm", "")
            .unwrap();
    let old_file = directory.join(format!("{}.worker.lock", record.task_id()));
    assert!(old_file.exists());
    drop(first);
    drop(second);
    let _second = store.worker_lock(&next).unwrap();
    assert!(!old_file.exists());
}
