use super::*;

/// 创建会话 API 测试路径。
fn test_paths(root: &std::path::Path) -> crate::paths::SaiPaths {
    crate::paths::SaiPaths {
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

#[tokio::test]
async fn session_workspace_id_uses_session_owner() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let active = temp.path().join("active");
    let owner = temp.path().join("owner");
    std::fs::create_dir_all(&active).unwrap();
    std::fs::create_dir_all(&owner).unwrap();

    crate::runtime_cwd::scope(active, async {
        let session =
            crate::state::create_session_for_workspace(&paths, &owner, Some("owned")).unwrap();

        let actual = session_workspace_id(&paths, &session.id).unwrap();
        // workspace_scope 会 canonicalize 路径，期望值需与落盘 ID 使用同一口径
        let expected = crate::platform::windows_path::canonicalize(&owner)
            .map(|path| crate::state::workspace_id_for_path(&path))
            .unwrap_or_else(|_| crate::state::workspace_id_for_path(&owner));
        assert_eq!(actual, expected);
    })
    .await;
}

/// 【Web会话】【Git 工作区】验证嵌套目录可识别上级 Git 仓库。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn workspace_git_flag_detects_repository_ancestors() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repository");
    let nested = repository.join("nested");
    let ordinary = temp.path().join("ordinary");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&ordinary).unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repository)
        .status()
        .unwrap();
    assert!(status.success());

    assert!(crate::web::workspace::is_git_repository(&nested).await);
    assert!(!crate::web::workspace::is_git_repository(&ordinary).await);
}

/// 【会话加载】【持有者心跳】未打开的会话不算加载，终端持有后才算加载。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn session_loaded_follows_alive_terminal_or_web_holder() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    crate::runtime_cwd::scope(workspace.clone(), async {
        let session =
            crate::state::create_session_for_workspace(&paths, &workspace, Some("held")).unwrap();
        assert_eq!(
            session_loaded_holder(&paths, &workspace, &session.id),
            (false, None)
        );

        let (_, state_dir) =
            crate::state::state_dir_for_workspace_session(&paths, &workspace, &session.id).unwrap();
        let _guard = crate::runner::SessionHolderGuard::acquire(
            &state_dir,
            &session.id,
            crate::runner::SessionOwner::Repl,
        )
        .unwrap();
        assert_eq!(
            session_loaded_holder(&paths, &workspace, &session.id),
            (true, Some("repl".to_string()))
        );
    })
    .await;
}
