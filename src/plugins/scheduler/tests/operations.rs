use super::super::{execute, get, process, store::Store, worker};
use super::{create, fixture, Launcher};
use sai_plugin_runtime::host::{
    ScheduleListOptions, ScheduleRequest, ScheduledStatus, SchedulerRequest, SchedulerResponse,
    SystemContext,
};
use std::time::Duration;

/// 【调度操作测试】【权限复核】宿主直接调用同样拒绝缺失授权和只读写入，拒绝前不创建任务目录。
#[tokio::test]
async fn host_rejects_unauthorized_mutations_before_io() {
    let (_root, paths, descriptor) = fixture();
    for (caps, allow_writes) in [(Default::default(), true), (descriptor.grants(), false)] {
        let result = execute(
            &paths,
            "schedule-fixture",
            Some(&descriptor.revision().unwrap()),
            SchedulerRequest::Schedule(ScheduleRequest {
                due_at: 1,
                command: "deliver".into(),
                arguments: String::new(),
            }),
            &SystemContext {
                workdir: paths.config_dir.display().to_string(),
                allow_writes,
            },
            &caps,
        )
        .await;
        assert!(result.is_err());
        assert!(!paths.state_dir.exists());
    }
}

/// 【调度操作测试】【发布前取消】阻塞线程排队期间丢弃调用，之后不得创建记录或工作进程。
#[test]
fn cancelling_a_queued_host_operation_prevents_late_publication() {
    let (_root, paths, descriptor) = fixture();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (release, blocked) = std::sync::mpsc::channel();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered.send(()).unwrap();
            blocked.recv().unwrap();
        });
        ready.await.unwrap();
        let revision = descriptor.revision().unwrap();
        let caps = descriptor.grants();
        let context = SystemContext {
            workdir: paths.config_dir.display().to_string(),
            allow_writes: true,
        };
        let mut operation = Box::pin(execute(
            &paths,
            "schedule-fixture",
            Some(&revision),
            SchedulerRequest::Schedule(ScheduleRequest {
                due_at: 1,
                command: "deliver".into(),
                arguments: String::new(),
            }),
            &context,
            &caps,
        ));
        std::future::poll_fn(|cx| {
            use std::future::Future;
            assert!(operation.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(operation);
        release.send(()).unwrap();
        blocker.await.unwrap();
        tokio::task::spawn_blocking(|| ()).await.unwrap();
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
    assert!(!paths.state_dir.exists());
}

/// 【调度操作测试】【分页】列表只返回请求范围，查询不会重新启动任何任务。
#[tokio::test]
async fn paged_queries_are_bounded_and_have_no_execution_side_effects() {
    let (_root, paths, descriptor) = fixture();
    let launcher = Launcher::default();
    for _ in 0..3 {
        create(&paths, &descriptor, "deliver", 253402300799, &launcher);
    }
    let all = super::super::list(&paths, "schedule-fixture").unwrap();
    let response = execute(
        &paths,
        "schedule-fixture",
        None,
        SchedulerRequest::List(ScheduleListOptions {
            offset: 1,
            limit: 1,
        }),
        &SystemContext::default(),
        &descriptor.grants(),
    )
    .await
    .unwrap();
    match response {
        SchedulerResponse::Tasks(tasks) => assert_eq!(tasks, all[1..2]),
        _ => panic!("unexpected response"),
    }
    assert_eq!(
        launcher.spawned.load(std::sync::atomic::Ordering::SeqCst),
        3
    );
}

/// 【调度操作测试】【取消确认】执行中的任务先进入取消中，工作入口释放回调后记录取消终态。
#[tokio::test]
async fn running_commands_observe_cancellation_and_next_jobs_recover() {
    let (_root, paths, descriptor) = fixture();
    let task = create(&paths, &descriptor, "hold", 1, &Launcher::default());
    let record = Store::open(&paths.state_dir, "schedule-fixture")
        .unwrap()
        .get(&task.id)
        .unwrap()
        .unwrap();
    // 1. 【调度操作测试】【开始观察】直接读取原子发布的记录，避免观察过程占用任务写入锁
    let (_, storage) = crate::plugins::private::paths::namespace(
        &paths.state_dir,
        "plugin-storage",
        "schedule-fixture",
        "",
    )
    .unwrap();
    let calls = storage.join(format!("{}.json", blake3::hash(b"calls").to_hex()));
    let copy = paths.clone();
    let id = task.id.clone();
    let run = tokio::spawn(async move {
        worker::run(&copy.state_dir, "schedule-fixture", &id, &record.launch).await
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let value = match std::fs::read(&calls) {
                Ok(bytes) => Some(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => panic!("read scheduled command observation: {error}"),
            };
            if value == Some(serde_json::json!(1)) {
                break;
            }
            assert!(
                !run.is_finished(),
                "scheduled command exited before publishing calls: {:?}",
                get(&paths, "schedule-fixture", &task.id)
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(super::super::cancel(&paths, "schedule-fixture", &task.id).unwrap());
    assert_eq!(
        get(&paths, "schedule-fixture", &task.id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Cancelling
    );
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        get(&paths, "schedule-fixture", &task.id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Cancelled
    );
    let store = Store::open(&paths.state_dir, "schedule-fixture").unwrap();
    assert!(process::claim(&store.directory, &task.id)
        .unwrap()
        .is_some());
    let next = create(&paths, &descriptor, "deliver", 1, &Launcher::default());
    let record = store.get(&next.id).unwrap().unwrap();
    worker::run(
        &paths.state_dir,
        "schedule-fixture",
        &next.id,
        &record.launch,
    )
    .await
    .unwrap();
    assert_eq!(
        get(&paths, "schedule-fixture", &next.id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Completed
    );
}

/// 【调度操作测试】【命令错误和输出】失败被记录，有界 Unicode 输出保留完整字符并标明截断。
#[tokio::test]
async fn worker_records_errors_and_truncates_outputs_at_utf8_boundaries() {
    let (_root, paths, descriptor) = fixture();
    for command in ["fail", "large"] {
        let task = create(&paths, &descriptor, command, 1, &Launcher::default());
        let record = Store::open(&paths.state_dir, "schedule-fixture")
            .unwrap()
            .get(&task.id)
            .unwrap()
            .unwrap();
        worker::run(
            &paths.state_dir,
            "schedule-fixture",
            &task.id,
            &record.launch,
        )
        .await
        .unwrap();
        let result = get(&paths, "schedule-fixture", &task.id).unwrap().unwrap();
        if command == "fail" {
            assert_eq!(result.status, ScheduledStatus::Failed);
            assert!(result.error.unwrap().contains("scheduled fixture failure"));
        } else {
            assert_eq!(result.status, ScheduledStatus::Completed);
            assert!(result.output_truncated);
            assert_eq!(result.output.unwrap(), "汉".repeat(5461));
        }
    }
}
