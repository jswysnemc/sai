use super::{
    super::{record::JobRecord, store::Store},
    Launcher,
};
use crate::{
    config::AppConfig,
    i18n::{with_locale, Locale},
    paths::SaiPaths,
    plugins::{self, discovery::PluginDescriptor, GrantChanges, GrantUpdate},
};
use sai_plugin_runtime::host::{ScheduleRequest, ScheduledStatus, SystemContext};
use serde_json::json;
use std::sync::atomic::AtomicBool;

/// 【调度语言测试】【知识库样本】无参数；返回隔离配置和使用当前语言构建的实际内置描述符
fn fixture() -> (tempfile::TempDir, SaiPaths, AppConfig, PluginDescriptor) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    let config = AppConfig::default();
    std::fs::write(&paths.config_file, serde_json::to_vec(&config).unwrap()).unwrap();
    let descriptor = plugins::tests::knowledge_support::descriptor(
        root.path(),
        &config,
        json!({"language":match crate::i18n::locale() { Locale::Zh => "zh", Locale::En => "en" }}),
    );
    (root, paths, config, descriptor)
}

/// 【调度语言测试】【正式任务】保留调度预检和持久化，只替换实际进程创建
/// @param paths 隔离目录；descriptor 为真实内置描述符
/// @returns 等待执行的知识库列表任务
fn create(paths: &SaiPaths, descriptor: &PluginDescriptor) -> JobRecord {
    let task = super::super::schedule(
        paths,
        "knowledge-base",
        &descriptor.revision().unwrap(),
        ScheduleRequest {
            due_at: 1,
            command: "list".into(),
            arguments: json!({"format":"json"}).to_string(),
        },
        &SystemContext {
            workdir: paths.config_dir.display().to_string(),
            allow_writes: true,
        },
        &Launcher::default(),
        &AtomicBool::new(false),
    )
    .unwrap();
    Store::open(&paths.state_dir, "knowledge-base")
        .unwrap()
        .get(&task.id)
        .unwrap()
        .unwrap()
}

/// 【调度语言测试】【跨语言恢复】中英创建与恢复分别使用不同环境，命令仍执行原快照
/// @returns 无；恢复预检和实际工作入口都成功，库按原业务创建
#[test]
fn knowledge_scheduled_commands_preserve_language_across_resume_and_dispatch() {
    for (parent, worker) in [(Locale::En, Locale::Zh), (Locale::Zh, Locale::En)] {
        let (_root, paths, _config, descriptor) = with_locale(parent, fixture);
        let record = with_locale(parent, || create(&paths, &descriptor));
        assert_eq!(record.language, Some(parent));
        with_locale(worker, || {
            assert!(
                super::super::load_current(&paths, "knowledge-base", &record.revision, None)
                    .is_ok()
            );
            super::super::resume(
                &paths,
                "knowledge-base",
                &record.task.id,
                &Launcher::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
            let resumed = Store::open(&paths.state_dir, "knowledge-base")
                .unwrap()
                .get(&record.task.id)
                .unwrap()
                .unwrap();
            assert_eq!(resumed.language, Some(parent));
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(super::super::worker::run(
                    &paths.state_dir,
                    "knowledge-base",
                    &resumed.task.id,
                    &resumed.launch,
                ))
                .unwrap();
        });
        let finished = super::super::get(&paths, "knowledge-base", &record.task.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            finished.status,
            ScheduledStatus::Completed,
            "{:?}",
            finished.error
        );
        assert_eq!(finished.output.as_deref(), Some("[]"));
        assert!(
            std::path::Path::new(descriptor.settings()["data_dir"].as_str().unwrap())
                .join("kb_meta.db")
                .exists()
        );
    }
}

/// 【调度语言测试】【到期撤权】语言一致性不能绕过实际停用、存储撤权和设置变化
/// @returns 无；失败发生在业务执行之前，不创建知识库
#[test]
fn knowledge_scheduled_language_keeps_configuration_and_grant_revalidation() {
    for change in ["disabled", "storage", "settings"] {
        let (_root, paths, config, descriptor) = with_locale(Locale::En, fixture);
        let record = with_locale(Locale::En, || create(&paths, &descriptor));
        with_locale(Locale::Zh, || {
            match change {
                "disabled" => plugins::set_enabled(
                    &config,
                    &paths,
                    "knowledge-base",
                    false,
                    GrantUpdate::Keep,
                )
                .unwrap(),
                "storage" => plugins::set_enabled(
                    &config,
                    &paths,
                    "knowledge-base",
                    true,
                    GrantUpdate::Changes(GrantChanges {
                        plugin_storage: Some(false),
                        ..Default::default()
                    }),
                )
                .unwrap(),
                _ => {
                    plugins::configure(
                        &config,
                        &paths,
                        "knowledge-base",
                        json!({"max_search_results":3}),
                    )
                    .unwrap();
                }
            }
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(super::super::worker::run(
                    &paths.state_dir,
                    "knowledge-base",
                    &record.task.id,
                    &record.launch,
                ))
                .unwrap();
        });
        let result = super::super::get(&paths, "knowledge-base", &record.task.id)
            .unwrap()
            .unwrap();
        assert_eq!(result.status, ScheduledStatus::Failed, "{change}");
        assert!(
            !std::path::Path::new(descriptor.settings()["data_dir"].as_str().unwrap()).exists()
        );
    }
}

/// 【调度语言测试】【旧记录兼容】缺省语言保留旧行为，非法语言不能进入调度
/// @returns 无；旧版本记录仍能通过归属校验，新记录只接受已支持的语言
#[test]
fn scheduled_language_accepts_legacy_records_and_rejects_unknown_values() {
    let (_root, paths, _config, descriptor) = fixture();
    let record = create(&paths, &descriptor);
    let mut value = serde_json::to_value(&record).unwrap();
    value.as_object_mut().unwrap().remove("language");
    let legacy: JobRecord = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(legacy.language, None);
    legacy.validate("knowledge-base", &record.task.id).unwrap();
    value["language"] = json!("unsupported");
    assert!(serde_json::from_value::<JobRecord>(value).is_err());
}
