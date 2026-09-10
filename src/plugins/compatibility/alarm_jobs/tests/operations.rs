use super::{fixture, save};
use crate::plugins::scheduler;
use sai_plugin_runtime::host::ScheduledStatus;

/// 【旧闹钟测试】【归属和取消】无 PID 记录可取消，其他插件无法查看或取消这条记录。
/// @returns 无，原文件保持逐字节一致
#[test]
fn legacy_cancel_preserves_original_file_and_plugin_ownership() {
    let (_root, paths, record) = fixture();
    let original = std::fs::read(paths.state_dir.join("alarms.json")).unwrap();
    let id = record.task_id();
    assert!(scheduler::list(&paths, "other").unwrap().is_empty());
    assert!(scheduler::get(&paths, "other", &id).unwrap().is_none());
    assert!(!scheduler::cancel(&paths, "other", &id).unwrap());
    assert!(scheduler::cancel(&paths, "alarm", &id).unwrap());
    assert!(!scheduler::cancel(&paths, "alarm", &id).unwrap());
    let task = scheduler::get(&paths, "alarm", &id).unwrap().unwrap();
    assert_eq!(task.status, ScheduledStatus::Cancelled);
    assert!(task.finished_at.is_some());
    assert_eq!(
        std::fs::read(paths.state_dir.join("alarms.json")).unwrap(),
        original
    );
    assert!(format!(
        "{:#}",
        scheduler::resume_task(&paths, "alarm", &id).unwrap_err()
    )
    .contains("legacy alarms cannot be resumed"));
}

/// 【旧闹钟测试】【状态身份】响铃变化不能复活已取消记录，业务参数变化不能继承原状态。
/// @returns 无
#[test]
fn legacy_state_is_bound_to_record_identity_but_not_old_status() {
    let (_root, paths, mut record) = fixture();
    let id = record.task_id();
    assert!(scheduler::cancel(&paths, "alarm", &id).unwrap());
    record.status = super::super::record::LegacyStatus::Ringing;
    save(&paths, std::slice::from_ref(&record));
    assert_eq!(
        scheduler::get(&paths, "alarm", &id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Cancelled
    );
    record.label = "新的记录内容".into();
    save(&paths, std::slice::from_ref(&record));
    assert_eq!(
        scheduler::get(&paths, "alarm", &id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Running
    );
}

/// 【旧闹钟测试】【取消竞争】并发请求最多一次成功，锁占用只按明确诊断重试。
/// @returns 无，最终状态完整且原文件没有修改
#[test]
fn legacy_concurrent_cancellation_has_one_successful_transition() {
    let (_root, paths, record) = fixture();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let (paths, id, barrier) = (paths.clone(), record.task_id(), barrier.clone());
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            for _ in 0..200 {
                match scheduler::cancel(&paths, "alarm", &id) {
                    Ok(changed) => return changed,
                    Err(error) if super::super::store::is_busy(&error) => {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(error) => panic!("{error:#}"),
                }
            }
            panic!("legacy cancellation lock did not become available");
        }));
    }
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum::<usize>(),
        1
    );
    assert_eq!(
        scheduler::get(&paths, "alarm", &record.task_id())
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Cancelled
    );
}

/// 【旧闹钟测试】【进程复用】普通进程不能作为旧闹钟取消，观察失败不会写入原文件。
/// @returns 无，测试进程不会收到信号
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[test]
fn legacy_mismatched_process_is_diagnosed_without_signalling() {
    let (_root, paths, mut record) = fixture();
    record.pid = Some(std::process::id());
    save(&paths, std::slice::from_ref(&record));
    let original = std::fs::read(paths.state_dir.join("alarms.json")).unwrap();
    let task = scheduler::get(&paths, "alarm", &record.task_id())
        .unwrap()
        .unwrap();
    assert_eq!(task.status, ScheduledStatus::Failed);
    assert!(task.error.unwrap().contains("identity changed"));
    assert!(scheduler::cancel(&paths, "alarm", &record.task_id()).is_err());
    assert_eq!(
        std::fs::read(paths.state_dir.join("alarms.json")).unwrap(),
        original
    );
}
