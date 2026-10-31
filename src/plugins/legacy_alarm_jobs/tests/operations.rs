use super::{fixture, save};
use crate::plugins::{legacy_alarm_jobs, scheduler};
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
    assert!(scheduler::get(&paths, "alarm", &id).unwrap().is_none());
    assert!(!scheduler::cancel(&paths, "alarm", &id).unwrap());
    assert!(legacy_alarm_jobs::cancel(&paths, &id).unwrap());
    assert!(!legacy_alarm_jobs::cancel(&paths, &id).unwrap());
    let task = legacy_alarm_jobs::get(&paths, &id).unwrap().unwrap();
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
    .contains("scheduled task not found"));
}

/// 【旧闹钟测试】【状态身份】响铃变化不能复活已取消记录，业务参数变化不能继承原状态。
/// @returns 无
#[test]
fn legacy_state_is_bound_to_record_identity_but_not_old_status() {
    let (_root, paths, mut record) = fixture();
    let id = record.task_id();
    assert!(legacy_alarm_jobs::cancel(&paths, &id).unwrap());
    record.status = super::super::record::LegacyStatus::Ringing;
    save(&paths, std::slice::from_ref(&record));
    assert_eq!(
        legacy_alarm_jobs::get(&paths, &id).unwrap().unwrap().status,
        ScheduledStatus::Cancelled
    );
    record.label = "新的记录内容".into();
    save(&paths, std::slice::from_ref(&record));
    assert_eq!(
        legacy_alarm_jobs::get(&paths, &id).unwrap().unwrap().status,
        ScheduledStatus::Running
    );
}

/// 【旧闹钟测试】【取消竞争】并发请求最多一次成功，锁占用只按明确诊断重试。
/// @returns 无，最终状态完整且原文件没有修改
#[test]
fn legacy_concurrent_cancellation_has_one_successful_transition() {
    for _ in 0..32 {
        assert_single_cancellation_transition(std::time::Duration::ZERO);
    }
}

/// 【旧闹钟测试】【慢锁回归】无参数，返回无；锁持有超过一秒后释放仍只允许一次取消成功。
#[test]
fn legacy_concurrent_cancellation_waits_for_delayed_lock_release() {
    assert_single_cancellation_transition(std::time::Duration::from_millis(1500));
}

/// 【旧闹钟测试】【取消竞争】从未创建的状态目录开始，同时取消同一任务。
/// 参数: lock_hold 为模拟外部锁持有时间，零值不预建状态目录；返回: 无，所有线程结束后检查唯一终态。
fn assert_single_cancellation_transition(lock_hold: std::time::Duration) {
    let (_root, paths, record) = fixture();
    let release = if lock_hold.is_zero() {
        None
    } else {
        let store = super::super::store::Store::open(&paths).unwrap();
        let lock = store.cancellation_lock().unwrap();
        Some(std::thread::spawn(move || {
            std::thread::sleep(lock_hold);
            drop(lock);
        }))
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let (paths, id, barrier) = (paths.clone(), record.task_id(), barrier.clone());
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            // 1. 【旧闹钟测试】【有界等待】使用真实截止时间，不能把慢文件同步误判为取消锁失效
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                match legacy_alarm_jobs::cancel(&paths, &id) {
                    Ok(changed) => return changed,
                    Err(error) if super::super::store::is_busy(&error) => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "legacy cancellation lock did not become available: {error:#}"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(10))
                    }
                    Err(error) => panic!("{error:#}"),
                }
            }
        }));
    }
    let results: Vec<_> = handles.into_iter().map(|handle| handle.join()).collect();
    if let Some(release) = release {
        release.join().unwrap();
    }
    assert_eq!(
        results
            .into_iter()
            .map(|result| usize::from(result.unwrap()))
            .sum::<usize>(),
        1
    );
    assert_eq!(
        legacy_alarm_jobs::get(&paths, &record.task_id())
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
    let task = legacy_alarm_jobs::get(&paths, &record.task_id())
        .unwrap()
        .unwrap();
    assert_eq!(task.status, ScheduledStatus::Failed);
    assert!(task.error.unwrap().contains("identity changed"));
    assert!(legacy_alarm_jobs::cancel(&paths, &record.task_id()).is_err());
    assert_eq!(
        std::fs::read(paths.state_dir.join("alarms.json")).unwrap(),
        original
    );
}
