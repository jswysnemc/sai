use super::super::{record::Flag, store::Store, worker, worker_runtime};
use super::{configure, fixture, save};
use sai_plugin_runtime::{host::ScheduledStatus, InvocationContext};
use serde_json::json;

/// 【旧闹钟测试】【工作入口身份】不同参数或不同 PID 不能承接已发布任务。
/// @returns 无，不调用声音后端
#[tokio::test]
async fn legacy_worker_rejects_forged_parameters_and_pid() {
    let (_root, paths, mut record) = fixture();
    configure(&paths, json!({"enabled":false}));
    record.pid = Some(std::process::id());
    save(&paths, std::slice::from_ref(&record));
    assert!(
        worker::run(&paths, &record.id, "different", &record.label, None)
            .await
            .is_err()
    );
    assert!(
        worker::run(&paths, &record.id, &record.time, "different", None)
            .await
            .is_err()
    );
    record.pid = Some(if std::process::id() == 1 { 2 } else { 1 });
    save(&paths, std::slice::from_ref(&record));
    assert!(
        worker::run(&paths, &record.id, &record.time, &record.label, None)
            .await
            .is_err()
    );
    assert!(Store::open(&paths)
        .unwrap()
        .flag(&record)
        .unwrap()
        .is_none());
}

/// 【旧闹钟测试】【绝对时间与撤权】按旧记录 due_at 执行授权复核，不重新解析倒计时。
/// @returns 无，禁用结果持久化为失败，第二次入口不会重试
#[tokio::test]
async fn legacy_worker_uses_due_at_and_records_current_disable_without_replay() {
    let (_root, paths, mut record) = fixture();
    configure(&paths, json!({"enabled":false}));
    record.pid = Some(std::process::id());
    record.due_at = 0;
    record.time = "not parsed in Rust".into();
    save(&paths, std::slice::from_ref(&record));
    let original = std::fs::read(paths.state_dir.join("alarms.json")).unwrap();
    worker::run(&paths, &record.id, &record.time, &record.label, None)
        .await
        .unwrap();
    let store = Store::open(&paths).unwrap();
    let flag = store.flag(&record).unwrap().unwrap();
    assert_eq!(flag.status, ScheduledStatus::Failed);
    assert!(flag.error.as_ref().unwrap().contains("disabled"));
    worker::run(&paths, &record.id, &record.time, &record.label, None)
        .await
        .unwrap();
    assert_eq!(store.flag(&record).unwrap().unwrap().error, flag.error);
    assert_eq!(
        std::fs::read(paths.state_dir.join("alarms.json")).unwrap(),
        original
    );
}

/// 【旧闹钟测试】【发布等待】入口只在原父进程发布对应 PID 后检查到期任务。
/// @returns 无，测试期间始终禁用真实通知
#[tokio::test]
async fn legacy_worker_waits_for_parent_publication() {
    let (_root, paths, mut record) = fixture();
    configure(&paths, json!({"enabled":false}));
    record.pid = Some(std::process::id());
    record.due_at = 0;
    save(&paths, &[]);
    let (result, ()) = tokio::join!(
        worker::run(&paths, &record.id, &record.time, &record.label, None),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            save(&paths, std::slice::from_ref(&record));
        }
    );
    result.unwrap();
    assert_eq!(
        Store::open(&paths)
            .unwrap()
            .flag(&record)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Failed
    );
}

/// 【旧闹钟测试】【等待中取消】绝对时间尚未到达时响应取消，退出前写入确认终态。
/// @returns 无，不加载或播放通知
#[tokio::test]
async fn legacy_worker_acknowledges_cancellation_while_waiting() {
    let (_root, paths, mut record) = fixture();
    configure(&paths, json!({"enabled":false}));
    record.pid = Some(std::process::id());
    save(&paths, std::slice::from_ref(&record));
    let store = Store::open(&paths).unwrap();
    let (result, ()) = tokio::join!(
        worker::run(&paths, &record.id, &record.time, &record.label, None),
        async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            store
                .transition(
                    &record,
                    Some(Flag::new(&record, ScheduledStatus::Cancelling, None).unwrap()),
                    &[ScheduledStatus::Scheduled],
                )
                .unwrap();
        }
    );
    result.unwrap();
    assert_eq!(
        store.flag(&record).unwrap().unwrap().status,
        ScheduledStatus::Cancelled
    );
}

/// 【旧闹钟测试】【禁止重放】已运行或终态记录不重新调用到期加载入口。
/// @returns 无
#[tokio::test]
async fn legacy_worker_does_not_replay_running_or_finished_records() {
    let (_root, paths, mut record) = fixture();
    configure(&paths, json!({"enabled":false}));
    record.pid = Some(std::process::id());
    record.due_at = 0;
    save(&paths, std::slice::from_ref(&record));
    let store = Store::open(&paths).unwrap();
    for next in [
        ScheduledStatus::Running,
        ScheduledStatus::Completed,
        ScheduledStatus::Failed,
        ScheduledStatus::Cancelled,
    ] {
        let current = store
            .flag(&record)
            .unwrap()
            .map(|flag| flag.status)
            .unwrap_or(ScheduledStatus::Scheduled);
        store
            .transition(
                &record,
                Some(Flag::new(&record, next, None).unwrap()),
                &[current],
            )
            .unwrap();
        worker::run(&paths, &record.id, &record.time, &record.label, None)
            .await
            .unwrap();
        let flag = store.flag(&record).unwrap().unwrap();
        assert_eq!(flag.status, next);
        assert!(flag.error.is_none());
    }
}

/// 【旧闹钟测试】【独立撤权】已有调度或通知撤权会阻止旧工作入口加载。
/// @returns 无
#[test]
fn legacy_worker_respects_current_scheduling_and_notification_grants() {
    let (_root, paths, record) = fixture();
    for system in [json!({"schedule":true}), json!({"notify":true}), json!({})] {
        configure(&paths, json!({"enabled":true,"grants":{"system":system}}));
        assert!(worker_runtime::load(&paths, &record).is_err());
    }
}

/// 【旧闹钟测试】【音频旧许可】默认兼容仅增加原文件路径，不扩大目录或持久配置。
/// @returns 无，不播放音频
#[test]
fn legacy_audio_inheritance_is_exact_and_local_to_verified_worker() {
    let (root, paths, mut record) = fixture();
    let audio = root.path().join("old.wav");
    std::fs::write(&audio, b"fixture").unwrap();
    record.audio_file = Some(dunce::canonicalize(&audio).unwrap());
    let runtime = worker_runtime::load(&paths, &record).unwrap();
    let declared = &runtime.manifest().capabilities.system.read_paths;
    assert_eq!(declared.len(), 2);
    assert!(declared.contains("."));
    assert!(declared.contains(
        dunce::simplified(record.audio_file.as_ref().unwrap())
            .to_str()
            .unwrap()
    ));
    assert!(!paths.config_dir.join("plugins.jsonc").exists());
    let discovered = crate::plugins::discover(&crate::config::AppConfig::default(), &paths);
    let normal = discovered
        .plugins
        .iter()
        .find(|plugin| plugin.package.manifest.id == "alarm")
        .unwrap();
    assert_eq!(normal.capabilities().system.read_paths.len(), 1);
}

/// 【旧闹钟测试】【音频撤权】显式授权或读取范围会覆盖旧音频默认许可。
/// @returns 无，音频读取在进入后端之前拒绝
#[tokio::test]
async fn legacy_audio_inheritance_never_overrides_explicit_read_scope() {
    let (root, paths, mut record) = fixture();
    let audio = root.path().join("old.wav");
    std::fs::write(&audio, b"fixture").unwrap();
    record.audio_file = Some(dunce::canonicalize(&audio).unwrap());
    for setting in [
        json!({"enabled":true,"grants":{"system":{"schedule":true,"notify":true}}}),
        json!({"enabled":true,"settings":{"audio_paths":[]}}),
    ] {
        configure(&paths, setting);
        let original = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
        let runtime = worker_runtime::load(&paths, &record).unwrap();
        let result = runtime
            .call_command(
                "deliver",
                &record.arguments().unwrap(),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                },
            )
            .await;
        let error = format!("{:#}", result.unwrap_err());
        assert!(error.contains("read") || error.contains("path"), "{error}");
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            original
        );
    }
}
