use super::*;
use serde_json::json;

/// 【Hooks】【来源夹具】参数为临时根目录，返回私有目录与最小应用配置
fn fixture(root: &std::path::Path) -> (crate::paths::SaiPaths, crate::config::AppConfig) {
    let paths = crate::paths::SaiPaths::for_tests(root);
    paths.create_dirs().unwrap();
    let mut config = crate::config::AppConfig::default();
    config.hooks.enabled = true;
    (paths, config)
}

#[tokio::test]
#[cfg(unix)]
async fn external_source_requires_review_and_changes_invalidate_it() {
    let root = tempfile::tempdir().unwrap();
    let (paths, config) = fixture(root.path());
    let file = paths.config_dir.join("hooks.json");
    let write = |reason| {
        std::fs::write(&file, json!({"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{
        "type":"command", "command":format!("printf '%s' '{{\"decision\":\"block\",\"reason\":\"{reason}\"}}'"), "timeout":1
    }]}]}}).to_string()).unwrap()
    };
    write("first");
    assert!(sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .is_empty());
    let source = sources::preview(&config, &paths, root.path())
        .unwrap()
        .remove(0);
    assert!(!source.trusted);
    assert!(trust::approve(&paths, &source, "wrong").is_err());
    trust::approve(&paths, &source, &source.digest).unwrap();
    let effective = sources::resolve(&config, &paths, root.path()).unwrap();
    assert_eq!(effective.items.len(), 1);
    let context = HookContext {
        session_id: "trust-test".into(),
        workdir: root.path().display().to_string(),
        tool_name: Some("read_file".into()),
        ..Default::default()
    };
    assert_eq!(
        dispatch_with_tools(&effective, HookEvent::PreToolUse, &context, None)
            .await
            .deny
            .as_deref(),
        Some("first")
    );
    write("changed");
    assert!(sources::revalidate(&effective.items[0]).is_err());
    assert!(sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .is_empty());
    assert!(crate::hooks::history::recent(&paths, Some("trust-test"))
        .iter()
        .any(|record| record.status == "completed"));
}

#[test]
fn monitored_scripts_and_plugin_content_are_revalidated() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("hooks.json");
    let script = root.path().join("notify.py");
    std::fs::write(&file, "{}").unwrap();
    std::fs::write(&script, "first").unwrap();
    let first = sources::fingerprint(&file, root.path(), &["notify.py".into()], false).unwrap();
    let plugin_first = sources::fingerprint(&file, root.path(), &[], true).unwrap();
    std::fs::write(&script, "changed").unwrap();
    assert_ne!(
        first,
        sources::fingerprint(&file, root.path(), &["notify.py".into()], false).unwrap()
    );
    assert_ne!(
        plugin_first,
        sources::fingerprint(&file, root.path(), &[], true).unwrap()
    );
    assert!(sources::fingerprint(&file, root.path(), &["../../missing".into()], false).is_err());
}

#[test]
fn malformed_untrusted_optional_source_does_not_block_startup() {
    let root = tempfile::tempdir().unwrap();
    let (paths, mut config) = fixture(root.path());
    config.hooks.enabled = false;
    std::fs::create_dir_all(root.path().join(".sai")).unwrap();
    std::fs::write(root.path().join(".sai/hooks.json"), "not valid json").unwrap();
    assert!(sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .is_empty());
    let source = sources::preview(&config, &paths, root.path())
        .unwrap()
        .remove(0);
    assert!(source.error.is_some());
    assert!(trust::approve(&paths, &source, "").is_err());
}

#[test]
#[cfg(unix)]
fn special_source_files_are_rejected_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let root = tempfile::tempdir().unwrap();
    let (paths, config) = fixture(root.path());
    let file = paths.config_dir.join("hooks.json");
    let name = std::ffi::CString::new(file.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let source = sources::preview(&config, &paths, root.path())
        .unwrap()
        .remove(0);
    assert!(source.error.as_deref().unwrap().contains("regular"));
    assert!(sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn output_budget_saves_full_text_without_serializing_runtime_metadata() {
    let root = tempfile::tempdir().unwrap();
    let (paths, mut config) = fixture(root.path());
    config.hooks.items.push(
        serde_json::from_value(
            json!({"name":"large", "event":"SessionStart", "script":"", "output_tokens":50}),
        )
        .unwrap(),
    );
    let hook = sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .remove(0);
    let original = "long output ".repeat(1000);
    let mut result = HookOutcome {
        context: vec![original.clone()],
        ..Default::default()
    };
    history::bound(&mut result, &hook);
    assert!(result.context[0].contains("Full hook output:"));
    let file = std::fs::read_dir(paths.data_dir.join("hook-output"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read_to_string(file).unwrap(), original);
    assert!(!serde_json::to_string(&hook)
        .unwrap()
        .contains("environment"));
}

#[tokio::test]
async fn session_end_is_emitted_only_once() {
    let root = tempfile::tempdir().unwrap();
    let (paths, mut config) = fixture(root.path());
    config.hooks.items.push(
        serde_json::from_value(json!({"name":"end", "event":"SessionEnd", "script":""})).unwrap(),
    );
    let hooks = sources::resolve(&config, &paths, root.path()).unwrap();
    let context = HookContext {
        session_id: "end-once-test".into(),
        ..Default::default()
    };
    lifecycle::register(hooks, context, crate::tools::ToolRegistry::new());
    assert!(lifecycle::started("end-once-test"));
    lifecycle::finish("end-once-test").await;
    lifecycle::finish("end-once-test").await;
    assert!(!lifecycle::started("end-once-test"));
    assert_eq!(
        history::recent(&paths, Some("end-once-test"))
            .iter()
            .filter(|record| record.status == "completed")
            .count(),
        1
    );
}

#[test]
fn execution_history_rotates_even_when_previous_file_exists() {
    let root = tempfile::tempdir().unwrap();
    let (paths, mut config) = fixture(root.path());
    config.hooks.items.push(
        serde_json::from_value(json!({"name":"rotate", "event":"Stop", "script":""})).unwrap(),
    );
    let hook = sources::resolve(&config, &paths, root.path())
        .unwrap()
        .items
        .remove(0);
    let directory = paths.data_dir.join("hook-history");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("events.previous.jsonl"), "older").unwrap();
    for _ in 0..2 {
        std::fs::write(directory.join("events.jsonl"), vec![b' '; 4_194_305]).unwrap();
        history::record(
            &hook,
            HookEvent::Stop,
            &HookContext::default(),
            "started",
            std::time::Instant::now(),
            None,
        );
        assert_eq!(history::recent(&paths, None).len(), 1);
    }
}

#[test]
fn managed_activation_does_not_reenable_user_hooks_on_repeated_resolution() {
    let root = tempfile::tempdir().unwrap();
    let (paths, mut config) = fixture(root.path());
    config.hooks.enabled = false;
    config.hooks.items.push(
        serde_json::from_value(json!({"name":"disabled", "event":"Stop", "script":"exit 2"}))
            .unwrap(),
    );
    config.hooks = sources::resolve(&config, &paths, root.path()).unwrap();
    config.hooks.enabled = true;
    config.hooks = sources::resolve(&config, &paths, root.path()).unwrap();
    assert!(!config.hooks.items[0].enabled);
    config.save(&paths).unwrap();
    let saved = crate::config::AppConfig::load(&paths).unwrap();
    assert!(!saved.hooks.enabled);
    assert!(saved.hooks.items[0].enabled);
}
