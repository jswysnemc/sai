use super::*;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use std::path::Path;

/// 创建隔离的会话数据测试路径。
///
/// 参数:
/// - `root`: 临时目录
///
/// 返回:
/// - 测试用 Sai 路径
fn test_paths(root: &Path) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    }
}

/// 验证统计包含结构化状态与顶层数据项。
#[tokio::test]
async fn session_data_summary_reports_state_contents() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    crate::runtime_cwd::scope(workspace.clone(), async {
        let session = crate::state::create_session(&paths, Some("managed")).unwrap();
        let store = StateStore::for_session(&paths, &session.id).unwrap();
        store.start_turn("turn-1", "question").unwrap();
        store.complete_turn("turn-1", "answer", None).unwrap();
        std::fs::write(store.state_dir().join("todos.json"), "[]").unwrap();
        drop(store);

        let workspace_id = crate::state::workspace_id_for_path(&workspace);
        let workspace_info = crate::web::workspaces::WorkspaceInfo {
            id: workspace_id.clone(),
            name: "managed".to_string(),
            path: workspace.display().to_string(),
            last_opened_at: String::new(),
        };
        let mut summaries = collect_session_data(&paths, &[workspace_info], &workspace_id).unwrap();
        todos::fill_counts(&paths, &mut summaries).await.unwrap();
        let summary = summaries.iter().find(|item| item.id == session.id).unwrap();

        assert_eq!(summary.turn_count, Some(1));
        assert_eq!(summary.todo_count, Some(0));
        assert!(summary.total_bytes > 0);
        assert!(summary
            .items
            .iter()
            .any(|item| item.name == "conversation.db"));
        assert!(summary.items.iter().any(|item| item.name == "todos.json"));
    })
    .await;
}

/// 验证清理只删除会话内容，并保留会话索引与标题。
#[tokio::test]
async fn clear_session_data_preserves_session_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let clear_workspace = workspace.clone();

    crate::runtime_cwd::scope(workspace, async move {
        let session = crate::state::create_session(&paths, Some("keep title")).unwrap();
        let store = StateStore::for_session(&paths, &session.id).unwrap();
        store.start_turn("turn-1", "question").unwrap();
        store.complete_turn("turn-1", "answer", None).unwrap();
        let marker = store.state_dir().join("nested/marker.txt");
        std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
        std::fs::write(&marker, "remove me").unwrap();
        drop(store);

        clear_session_data_for_workspace(&paths, &clear_workspace, &session.id).unwrap();

        let metadata = crate::state::list_sessions(&paths)
            .unwrap()
            .into_iter()
            .find(|item| item.id == session.id)
            .unwrap();
        assert_eq!(metadata.title, "keep title");
        assert!(!marker.exists());
        let reopened = StateStore::for_session(&paths, &session.id).unwrap();
        assert!(reopened.load_all_turns().unwrap().is_empty());
        assert!(reopened.state_dir().join("usage.json").is_file());
        assert!(reopened.state_dir().join("profile.md").is_file());
    })
    .await;
}

/// 会话数据面板横跨所有工作区，删除必须落到会话真正所属的作用域。
///
/// 回归此前的缺陷：删除只按会话 ID 在服务端当前工作区的索引里查找，
/// 别的工作区的会话找不到就静默返回成功，界面刷新后会话原样还在。
#[tokio::test]
async fn deletes_sessions_from_a_non_current_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let workspace_a = temp.path().join("workspace-a");
    let workspace_b = temp.path().join("workspace-b");
    std::fs::create_dir_all(&workspace_a).unwrap();
    std::fs::create_dir_all(&workspace_b).unwrap();

    // 1. 在工作区 B 里建一个会话
    let session_b = crate::runtime_cwd::scope(workspace_b.clone(), async {
        crate::state::create_session(&paths, Some("b session")).unwrap()
    })
    .await;

    // 2. 把当前工作区切到 A，此时 B 的会话不在当前作用域的索引里
    let target_b = session_b.id.clone();
    let deleted = crate::runtime_cwd::scope(workspace_a.clone(), async {
        let stale = crate::state::delete_sessions(&paths, &[target_b.clone()]).unwrap();
        assert!(
            stale.is_empty(),
            "按当前工作区删除跨工作区会话本就删不掉，这里固定住该前提"
        );
        crate::state::delete_sessions_for_workspace(&paths, &workspace_b, &[target_b.clone()])
            .unwrap()
    })
    .await;

    // 3. 指定工作区后必须真的删掉
    assert_eq!(deleted, vec![session_b.id.clone()]);
    let remaining = crate::state::list_sessions_for_workspace(&paths, &workspace_b).unwrap();
    assert!(
        !remaining.iter().any(|item| item.id == session_b.id),
        "会话应已从所属工作区的索引中移除"
    );
}
