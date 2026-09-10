use super::super::{record::JobRecord, store::Store};
use super::fixture;
use sai_plugin_runtime::host::{ScheduleRequest, MAX_ACTIVE_TASKS, MAX_SCHEDULED_TASKS};

/// 【调度配额测试】【活动与历史】活动任务不可驱逐，历史达到上限时只删除最旧终态记录。
#[test]
fn active_quota_and_terminal_history_are_bounded() {
    let (_root, paths, descriptor) = fixture();
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    let _lock = store.lock().unwrap();
    let mut records = Vec::new();
    for index in 0..MAX_SCHEDULED_TASKS {
        let mut record = JobRecord::new(
            "schedule-fixture",
            &descriptor.revision().unwrap(),
            ScheduleRequest {
                due_at: 1,
                command: "deliver".into(),
                arguments: String::new(),
            },
            &paths,
            paths.config_dir.display().to_string(),
        );
        record.task.created_at = 100 + index as i64;
        if index >= MAX_ACTIVE_TASKS {
            record.finish(None);
        }
        store.write(&record).unwrap();
        records.push(record);
    }
    assert!(store.reserve().is_err());
    records[0].finish(None);
    store.write(&records[0]).unwrap();
    store.reserve().unwrap();
    assert!(store.get(&records[0].task.id).unwrap().is_none());
    assert_eq!(store.list().unwrap().len(), MAX_SCHEDULED_TASKS - 1);
    assert!(store
        .get(&records[1].task.id)
        .unwrap()
        .unwrap()
        .task
        .status
        .is_active());
}

/// 【调度配额测试】【目录上限】大量未知文件不能使扫描提前结束并绕过任务配额。
#[test]
fn unexpected_directory_growth_is_rejected_instead_of_truncated() {
    let (_root, paths, _descriptor) = fixture();
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    for index in 0..270 {
        store
            .directory
            .write(format!(".unknown-{index}"), b"")
            .unwrap();
    }
    assert!(store.list().is_err());
}

/// 【调度配额测试】【记录边界】过大、损坏和跨插件复制的记录均不能进入执行状态。
#[test]
fn records_reject_oversize_corruption_and_foreign_ownership() {
    let (_root, paths, descriptor) = fixture();
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    let record = JobRecord::new(
        "schedule-fixture",
        &descriptor.revision().unwrap(),
        ScheduleRequest {
            due_at: 1,
            command: "deliver".into(),
            arguments: String::new(),
        },
        &paths,
        paths.config_dir.display().to_string(),
    );
    let filename = format!("{}.json", record.task.id);
    for bytes in [vec![b' '; 256 * 1024 + 1], b"not json".to_vec()] {
        store.directory.write(&filename, &bytes).unwrap();
        assert!(store.get(&record.task.id).is_err());
    }
    let other = Store::open(&paths.state_dir, "other-plugin").unwrap();
    other
        .directory
        .write(&filename, serde_json::to_vec(&record).unwrap())
        .unwrap();
    assert!(other.get(&record.task.id).is_err());
}
