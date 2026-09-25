use super::*;
use crate::{
    config::AppConfig,
    plugins::{todo_view::TodoView, GrantUpdate},
    web::workspaces::WorkspaceInfo,
};
use serde_json::{json, Value};

/// 【会话待办测试】【工作区描述】构造不改变进程当前目录的工作区统计输入
/// @param path 工作区路径
/// @returns 完整界面工作区描述
fn workspace(path: &FilePath) -> WorkspaceInfo {
    WorkspaceInfo {
        id: crate::state::workspace_id_for_path(path),
        name: "test".into(),
        path: path.display().to_string(),
        last_opened_at: String::new(),
    }
}

/// 【会话待办测试】【旧条目】构造保留原字段的活动项
/// @returns 一个未完成条目
fn item() -> Value {
    json!({"id":"a","text":"pending","status":"pending","created_at":"old","updated_at":"old"})
}

/// 【会话待办测试】【显式导入】普通安装并授权示例，通过公开命令导入测试状态
/// @param paths 隔离应用目录；store 为真实会话；work 为所属工作区
/// @returns 公共记录路径，供损坏状态及清理边界断言使用
async fn import_plan(paths: &SaiPaths, store: &StateStore, work: &FilePath) -> PathBuf {
    let config = AppConfig::default();
    if !paths.config_dir.join("plugins/todo").exists() {
        let source = FilePath::new(env!("CARGO_MANIFEST_DIR")).join("examples/lua-plugins/todo");
        crate::plugins::install(&source, paths, false).unwrap();
        crate::plugins::set_enabled(&config, paths, "todo", true, GrantUpdate::Declared).unwrap();
    }
    let scope = store.state_dir().display().to_string();
    crate::runtime_cwd::scope(work.to_path_buf(), async {
        let mut registry = crate::tools::builtin_registry_without_mcp(&config, paths);
        assert!(registry.plugin_diagnostics().is_empty());
        registry.start_plugin_session(store.session_id()).unwrap();
        registry.inherit_plugin_storage_session(&scope);
        let command = registry.plugin_command("todo", "import").unwrap();
        let name = command.name.clone();
        registry.register(command);
        let arguments = json!({"state":{"version":0,"items":[item()],"history":[]}});
        registry
            .call(
                &name,
                &json!({"arguments":arguments.to_string()}).to_string(),
            )
            .await
            .unwrap();
    })
    .await;
    paths
        .state_dir
        .join("plugin-state")
        .join(blake3::hash(b"todo").to_hex().to_string())
        .join(blake3::hash(scope.as_bytes()).to_hex().to_string())
        .join(format!("{}.json", blake3::hash(b"plan").to_hex()))
}

/// 【会话待办测试】【统计与错误】原生待办文件决定数量，损坏文件保留局部错误
/// @returns 无
#[tokio::test]
async fn session_data_todo_counts_follow_native_file_and_preserve_parse_errors() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let work = root.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let info = workspace(&work);
    crate::runtime_cwd::scope(work.clone(), async {
        let session = crate::state::create_session(&paths, Some("todo")).unwrap();
        let store = StateStore::for_session(&paths, &session.id).unwrap();
        let mut absent = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut absent).await.unwrap();
        assert!(absent.iter().all(|summary| summary.todo_count == Some(0)));
        std::fs::write(
            store.state_dir().join("todos.json"),
            json!([item()]).to_string(),
        )
        .unwrap();
        let mut summaries = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        let summary = summaries.iter().find(|item| item.id == session.id).unwrap();
        assert_eq!(summary.todo_count, Some(1));
        assert!(summary.items.iter().any(|item| item.name == "todos.json"));
        assert_eq!(
            summary.total_bytes,
            summary.items.iter().map(|item| item.bytes).sum::<u64>()
        );
        std::fs::write(store.state_dir().join("todos.json"), "{broken").unwrap();
        let mut summaries = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        let summary = summaries.iter().find(|item| item.id == session.id).unwrap();
        assert_eq!(summary.todo_count, None);
        assert!(summary
            .state_error
            .as_deref()
            .unwrap()
            .contains("failed to parse todo file"));
    })
    .await;
}

/// 【会话待办测试】【整体清理】清空会话数据删除新旧待办，其他工作区同名会话保持不变
/// @returns 无；重新读取不能再次导入已经清理的旧文件
#[tokio::test]
async fn session_data_clear_removes_todo_files_only_for_selected_workspace() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut stores = Vec::new();
    for name in ["first", "second"] {
        let work = root.path().join(name);
        std::fs::create_dir(&work).unwrap();
        let store = StateStore::for_workspace_session(&paths, &work, "default").unwrap();
        std::fs::write(
            store.state_dir().join("todos.json"),
            json!([item()]).to_string(),
        )
        .unwrap();
        import_plan(&paths, &store, &work).await;
        let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
        assert_eq!(
            view.snapshot(store.session_id(), store.state_dir(), &work)
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        // 1. 【会话待办测试】【清理前释放】仅保留路径，先关闭数据库句柄再删除状态目录
        stores.push((work, store.state_dir().to_path_buf()));
    }
    clear_session_data_for_workspace(&paths, &stores[0].0, "default").unwrap();
    for file in ["todos.json", "todos.history.json", "todos.plugin.json"] {
        assert!(!stores[0].1.join(file).exists());
    }
    let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
    assert!(view
        .snapshot("default", &stores[0].1, &stores[0].0)
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        view.snapshot("default", &stores[1].1, &stores[1].0)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}
