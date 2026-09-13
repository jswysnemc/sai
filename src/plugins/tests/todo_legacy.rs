use super::todo_support::{call, context, import, installed, item, record_path, snapshot};
use crate::{config::AppConfig, plugins::todo_view::TodoView, state::StateStore};
use serde_json::json;

/// 【待办导入测试】【旧记录接续】只有显式导入才接续旧条目，清空对话遵守公共状态清理规则
/// @returns 无；旧活动清单、历史和原插件快照均保持原文
#[tokio::test]
async fn todo_explicit_legacy_import_obeys_session_reset_and_preserves_sources() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let items = json!([item("a", "pending"), item("b", "pending")]);
    let history = json!([{"archived_at":"old-archive","items":[item("finished","completed")]}]);
    let old = json!({"version":1,"items":items,"history":history});
    for (name, value) in [
        ("todos.json", &items),
        ("todos.history.json", &history),
        ("todos.plugin.json", &old),
    ] {
        std::fs::write(store.state_dir().join(name), value.to_string()).unwrap();
    }
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await,
        json!({"items":[],"history":[]})
    );
    import(&runtime, root.path(), &scope, old.clone()).await;
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["history"],
        history
    );
    call(
        &runtime,
        root.path(),
        &scope,
        json!({"action":"update","index":1,"status":"completed"}),
    )
    .await;
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["items"][0]["status"],
        "completed"
    );
    store.reset_conversation().unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await,
        json!({"items":[],"history":[]})
    );
    import(&runtime, root.path(), &scope, old.clone()).await;
    for (name, value) in [
        ("todos.json", &items),
        ("todos.history.json", &history),
        ("todos.plugin.json", &old),
    ] {
        assert_eq!(
            std::fs::read_to_string(store.state_dir().join(name)).unwrap(),
            value.to_string()
        );
    }
}

/// 【待办导入测试】【界面归档】导入完成计划后，Web 与工具读取同一公共记录
/// @returns 无；重复查询只保留一份归档
#[tokio::test]
async fn todo_web_view_archives_explicitly_imported_completion_once() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    import(
        &runtime,
        root.path(),
        &scope,
        json!({"version":0,"items":[item("done","completed")],"history":[]}),
    )
    .await;
    let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
    let first = serde_json::to_value(
        view.snapshot(store.session_id(), store.state_dir(), root.path())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["items"], json!([]));
    assert_eq!(first["history"].as_array().unwrap().len(), 1);
    assert_eq!(first["history"][0]["items"][0]["created_at"], "old-created");
    let second = serde_json::to_value(
        view.snapshot(store.session_id(), store.state_dir(), root.path())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first, second);
}

/// 【待办导入测试】【覆盖保护】已有活动条目或历史时拒绝导入，清理后也不自动重读旧文件
/// @returns 无；空快照初始化允许导入，非空记录和旧文件不会被覆盖
#[tokio::test]
async fn todo_import_rejects_overwrite_and_never_implicitly_reimports() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let scope = "explicit-import";
    let value = json!({"version":1,"items":[item("a","pending")],"history":[]});
    snapshot(&runtime, root.path(), scope).await;
    import(&runtime, root.path(), scope, value.clone()).await;
    for archived in [false, true] {
        if archived {
            call(
                &runtime,
                root.path(),
                scope,
                json!({"action":"update","index":1,"status":"completed"}),
            )
            .await;
        }
        let saved = std::fs::read(record_path(&paths, scope)).unwrap();
        let result = runtime
            .call_command(
                "import",
                &json!({"state":value}).to_string(),
                context(root.path(), scope, true),
            )
            .await;
        assert!(format!("{:#}", result.unwrap_err()).contains("empty plan and history"));
        assert_eq!(std::fs::read(record_path(&paths, scope)).unwrap(), saved);
    }
    crate::plugins::clear_session_storage(&paths.state_dir, scope).unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), scope).await,
        json!({"items":[],"history":[]})
    );
}

/// 【待办导入测试】【损坏来源】非法 JSON、版本和条目不会写入计划或修改导入文件
/// @returns 无；修正文件后同一实例可完成显式导入
#[tokio::test]
async fn todo_import_validates_authorized_files_without_rewriting_them() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = installed(root.path());
    let directory = root.path().join(".sai/todo-import");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("snapshot.json");
    for text in [
        "{",
        "{}",
        "[]",
        r#"{"version":2,"items":[],"history":[]}"#,
        r#"{"version":1,"items":[{}],"history":[]}"#,
    ] {
        std::fs::write(&path, text).unwrap();
        assert!(runtime
            .call_command(
                "import",
                &json!({"path":path}).to_string(),
                context(root.path(), "import", true)
            )
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert!(!record_path(&paths, "import").exists());
    }
    let valid = json!({"version":0,"items":[item("old","pending")],"history":[]});
    std::fs::write(&path, valid.to_string()).unwrap();
    runtime
        .call_command(
            "import",
            &json!({"path":path}).to_string(),
            context(root.path(), "import", true),
        )
        .await
        .unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), "import").await["items"],
        valid["items"]
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), valid.to_string());
}
