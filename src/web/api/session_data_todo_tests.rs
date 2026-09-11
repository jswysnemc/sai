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

/// 【会话待办测试】【统计与错误】首次导入文件必须进入数据统计，损坏状态保留局部错误
/// @returns 无；禁用待办后仍可访问会话数据面板
#[tokio::test]
async fn session_data_todo_counts_follow_lua_and_preserve_query_errors() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let work = root.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let info = workspace(&work);
    crate::runtime_cwd::scope(work.clone(), async {
        let session = crate::state::create_session(&paths, Some("todo")).unwrap();
        let store = StateStore::for_session(&paths, &session.id).unwrap();
        std::fs::write(
            store.state_dir().join("todos.json"),
            json!([item()]).to_string(),
        )
        .unwrap();
        let mut summaries = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        let summary = summaries.iter().find(|item| item.id == session.id).unwrap();
        assert_eq!(summary.todo_count, Some(1));
        assert!(summary
            .items
            .iter()
            .any(|item| item.name == "todos.plugin.json"));
        assert_eq!(
            summary.total_bytes,
            summary.items.iter().map(|item| item.bytes).sum::<u64>()
        );
        std::fs::write(store.state_dir().join("todos.plugin.json"), "{broken").unwrap();
        let mut summaries = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        let summary = summaries.iter().find(|item| item.id == session.id).unwrap();
        assert_eq!(summary.todo_count, None);
        assert!(summary
            .state_error
            .as_deref()
            .unwrap()
            .contains("decode plugin storage record"));
        crate::plugins::set_enabled(
            &AppConfig::default(),
            &paths,
            "todo",
            false,
            GrantUpdate::Keep,
        )
        .unwrap();
        let mut summaries = collect_session_data(&paths, &[info.clone()], &info.id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        assert!(summaries
            .iter()
            .all(|summary| summary.todo_count == Some(0)));
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
        let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
        assert_eq!(
            view.snapshot(store.session_id(), store.state_dir(), &work)
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        stores.push((work, store));
    }
    clear_session_data_for_workspace(&paths, &stores[0].0, "default").unwrap();
    for file in ["todos.json", "todos.history.json", "todos.plugin.json"] {
        assert!(!stores[0].1.state_dir().join(file).exists());
    }
    let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
    assert!(view
        .snapshot("default", stores[0].1.state_dir(), &stores[0].0)
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        view.snapshot("default", stores[1].1.state_dir(), &stores[1].0)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}
