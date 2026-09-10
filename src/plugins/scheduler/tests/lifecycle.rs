use super::super::{get, store::Store, worker};
use super::{create, fixture, Launcher};
use sai_plugin_runtime::host::ScheduledStatus;

/// 【调度生命周期测试】【执行与去重】真实 Lua 命令完成后保存结果，重复工作入口不能再次执行。
#[tokio::test]
async fn worker_executes_a_persisted_command_once() {
    let (_root, paths, descriptor) = fixture();
    let task = create(&paths, &descriptor, "deliver", 1, &Launcher::default());
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
    let finished = get(&paths, "schedule-fixture", &task.id).unwrap().unwrap();
    assert_eq!(finished.status, ScheduledStatus::Completed);
    assert_eq!(finished.output.as_deref(), Some("双语 payload"));
    worker::run(
        &paths.state_dir,
        "schedule-fixture",
        &task.id,
        &record.launch,
    )
    .await
    .unwrap();
    let host = crate::plugins::private::PrivatePluginHost::new(&paths, "schedule-fixture");
    use sai_plugin_runtime::host::{PluginHost, StorageRequest};
    assert_eq!(
        host.plugin_storage(
            StorageRequest::Get {
                key: "calls".into()
            },
            &descriptor.grants(),
            false
        )
        .unwrap(),
        serde_json::json!(1)
    );
}

/// 【调度生命周期测试】【重新授权】禁用、撤权和源码修改后，到期任务必须拒绝执行。
#[tokio::test]
async fn worker_revalidates_enablement_grants_and_source_at_due_time() {
    for change in ["disabled", "revoked", "source"] {
        let (_root, paths, descriptor) = fixture();
        let task = create(&paths, &descriptor, "deliver", 1, &Launcher::default());
        let record = Store::open(&paths.state_dir, "schedule-fixture")
            .unwrap()
            .get(&task.id)
            .unwrap()
            .unwrap();
        if change == "source" {
            std::fs::write(
                paths.config_dir.join("plugins/schedule-fixture/init.lua"),
                "error('changed source must not run')",
            )
            .unwrap();
        } else {
            let config = paths.config_dir.join("plugins.jsonc");
            let mut value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
            if change == "disabled" {
                value["plugins"]["schedule-fixture"]["enabled"] = serde_json::json!(false);
            } else {
                value["plugins"]["schedule-fixture"]["grants"]["system"]["schedule"] =
                    serde_json::json!(false);
            }
            std::fs::write(config, serde_json::to_vec(&value).unwrap()).unwrap();
        }
        worker::run(
            &paths.state_dir,
            "schedule-fixture",
            &task.id,
            &record.launch,
        )
        .await
        .unwrap();
        assert_eq!(
            get(&paths, "schedule-fixture", &task.id)
                .unwrap()
                .unwrap()
                .status,
            ScheduledStatus::Failed,
            "{change}"
        );
        assert!(!paths.state_dir.join("plugin-storage").exists());
    }
}

/// 【调度生命周期测试】【调用时限】后台命令仍受清单总时长限制，超时不会留在运行中。
#[tokio::test]
async fn scheduled_commands_keep_their_callback_timeout() {
    let (_root, paths, mut descriptor) = fixture();
    descriptor.package.manifest.limits.timeout_ms = 100;
    std::fs::write(
        paths
            .config_dir
            .join("plugins/schedule-fixture/sai-plugin.json"),
        serde_json::to_vec(&descriptor.package.manifest).unwrap(),
    )
    .unwrap();
    let task = create(&paths, &descriptor, "hold", 1, &Launcher::default());
    let record = Store::open(&paths.state_dir, "schedule-fixture")
        .unwrap()
        .get(&task.id)
        .unwrap()
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        worker::run(
            &paths.state_dir,
            "schedule-fixture",
            &task.id,
            &record.launch,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let result = get(&paths, "schedule-fixture", &task.id).unwrap().unwrap();
    assert_eq!(result.status, ScheduledStatus::Failed);
    assert!(result.error.unwrap().contains("timed out"));
}

/// 【调度生命周期测试】【等待取消与身份】错误启动身份和提前取消都不能进入业务命令。
#[tokio::test]
async fn launch_identity_and_pending_cancellation_prevent_dispatch() {
    let (_root, paths, descriptor) = fixture();
    let task = create(
        &paths,
        &descriptor,
        "deliver",
        253402300799,
        &Launcher::default(),
    );
    let record = Store::open(&paths.state_dir, "schedule-fixture")
        .unwrap()
        .get(&task.id)
        .unwrap()
        .unwrap();
    worker::run(
        &paths.state_dir,
        "schedule-fixture",
        &task.id,
        "wrong-launch",
    )
    .await
    .unwrap();
    assert_eq!(
        get(&paths, "schedule-fixture", &task.id)
            .unwrap()
            .unwrap()
            .status,
        ScheduledStatus::Scheduled
    );
    assert!(super::super::cancel(&paths, "schedule-fixture", &task.id).unwrap());
    worker::run(
        &paths.state_dir,
        "schedule-fixture",
        &task.id,
        &record.launch,
    )
    .await
    .unwrap();
    assert!(!paths.state_dir.join("plugin-storage").exists());
}
