use super::super::{cancel, get, list, process, resume, store::Store};
use super::{create, fixture, Launcher};
use sai_plugin_runtime::host::ScheduledStatus;
use std::sync::atomic::{AtomicBool, Ordering};

/// 【调度存储测试】【隔离与持久化】重建存储保留任务，其他插件不能读取或取消。
#[test]
fn jobs_survive_reopening_and_are_isolated_by_plugin() {
    let (_root, paths, descriptor) = fixture();
    let launcher = Launcher::default();
    let task = create(&paths, &descriptor, "deliver", 253402300799, &launcher);
    assert_eq!(launcher.spawned.load(Ordering::SeqCst), 1);
    assert_eq!(
        get(&paths, "schedule-fixture", &task.id).unwrap(),
        Some(task.clone())
    );
    assert!(get(&paths, "other-plugin", &task.id).unwrap().is_none());
    assert!(!cancel(&paths, "other-plugin", &task.id).unwrap());
    assert!(cancel(&paths, "schedule-fixture", &task.id).unwrap());
    assert_eq!(
        get(&paths, "schedule-fixture", &task.id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Cancelled
    );
    assert!(!cancel(&paths, "schedule-fixture", &task.id).unwrap());
}

/// 【调度存储测试】【启动失败】失败记录可以观察，不能报告为已启动或丢失任务。
#[test]
fn spawn_failure_is_persisted_without_success() {
    let (_root, paths, descriptor) = fixture();
    let launcher = Launcher {
        fail: true,
        ..Default::default()
    };
    let result = super::super::schedule(
        &paths,
        "schedule-fixture",
        &descriptor.revision().unwrap(),
        sai_plugin_runtime::host::ScheduleRequest {
            due_at: 1,
            command: "deliver".into(),
            arguments: String::new(),
        },
        &sai_plugin_runtime::host::SystemContext {
            workdir: paths.config_dir.display().to_string(),
            allow_writes: true,
        },
        &launcher,
        &AtomicBool::new(false),
    );
    assert!(result.is_err());
    let tasks = list(&paths, "schedule-fixture").unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, ScheduledStatus::Failed);
    assert!(tasks[0]
        .error
        .as_ref()
        .unwrap()
        .contains("fixture spawn failed"));
}

/// 【调度存储测试】【独占恢复】活跃执行锁禁止重复启动，已经开始的中断任务不能重新执行。
#[test]
fn recovery_never_replays_started_work_or_duplicates_a_live_worker() {
    let (_root, paths, descriptor) = fixture();
    let launcher = Launcher::default();
    let task = create(&paths, &descriptor, "deliver", 1, &launcher);
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    let lease = process::claim(&store.directory, &task.id).unwrap().unwrap();
    assert!(resume(
        &paths,
        "schedule-fixture",
        &task.id,
        &launcher,
        &AtomicBool::new(false)
    )
    .is_err());
    drop(lease);
    resume(
        &paths,
        "schedule-fixture",
        &task.id,
        &launcher,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(launcher.spawned.load(Ordering::SeqCst), 2);
    {
        let _lock = store.lock().unwrap();
        let mut record = store.get(&task.id).unwrap().unwrap();
        record.task.status = ScheduledStatus::Running;
        store.write(&record).unwrap();
    }
    let stopped = resume(
        &paths,
        "schedule-fixture",
        &task.id,
        &launcher,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(stopped.status, ScheduledStatus::Failed);
    assert_eq!(launcher.spawned.load(Ordering::SeqCst), 2);
}

/// 【调度存储测试】【真实文件边界】符号链接和管道不能作为任务记录或执行锁。
#[cfg(unix)]
#[test]
fn record_and_execution_lock_reject_links_and_fifos() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let (root, paths, descriptor) = fixture();
    let task = create(&paths, &descriptor, "deliver", 1, &Launcher::default());
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    let outsider = root.path().join("outside");
    std::fs::write(&outsider, b"sentinel").unwrap();
    store
        .directory
        .remove_file(format!("{}.json", task.id))
        .unwrap();
    let record_path = paths
        .state_dir
        .join("plugin-jobs")
        .join(blake3::hash(b"schedule-fixture").to_hex().to_string())
        .join(blake3::hash(b"").to_hex().to_string())
        .join(format!("{}.json", task.id));
    symlink(&outsider, &record_path).unwrap();
    assert!(store.get(&task.id).is_err());
    std::fs::remove_file(&record_path).unwrap();
    let name = std::ffi::CString::new(record_path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(store.get(&task.id).is_err());
    store
        .directory
        .remove_file(format!("{}.lock", task.id))
        .unwrap();
    symlink(&outsider, record_path.with_extension("lock")).unwrap();
    assert!(process::claim(&store.directory, &task.id).is_err());
    assert_eq!(std::fs::read(&outsider).unwrap(), b"sentinel");
}
