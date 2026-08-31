use crate::paths::SaiPaths;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceScope {
    pub state_dir: PathBuf,
}

/// 返回当前工作区会话作用域。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前工作区作用域
pub fn current_workspace_scope(paths: &SaiPaths) -> Result<WorkspaceScope> {
    let cwd = crate::runtime_cwd::current_dir()?;
    Ok(workspace_scope_for_path(paths, &cwd))
}

/// 根据指定目录返回工作区会话作用域。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
///
/// 返回:
/// - 工作区作用域
pub fn workspace_scope_for_path(paths: &SaiPaths, workspace_path: &Path) -> WorkspaceScope {
    let workspace_id = workspace_id_for_directory(workspace_path);
    WorkspaceScope {
        state_dir: paths
            .state_dir
            .join("sessions")
            .join("workspaces")
            .join(workspace_id),
    }
}

/// 返回当前工作区的稳定 ID。
///
/// 与会话落盘走同一套规范化，因此可以直接和 `list_all_sessions` 报出的
/// `workspace_id` 比较。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前工作区 ID
pub fn current_workspace_id() -> Result<String> {
    let cwd = crate::runtime_cwd::current_dir()?;
    Ok(workspace_id_for_directory(&cwd))
}

/// 规范化工作区路径后生成稳定 ID。
///
/// 会话按这个 ID 分目录存放，任何按当前工作区查会话的地方都必须用它，不能
/// 直接哈希原始路径：Windows 上原始 cwd 可能是 8.3 短名（`RUNNER~1`）、大小写
/// 不一致或带 `\\?\` 前缀，规范化前后哈希值不同，查出来就是空列表。Linux 上
/// 对应 cwd 含符号链接的情况。
///
/// 参数:
/// - `workspace_path`: 工作区目录
///
/// 返回:
/// - 工作区 ID
fn workspace_id_for_directory(workspace_path: &Path) -> String {
    let normalized = crate::platform::windows_path::canonicalize(workspace_path)
        .unwrap_or_else(|_| workspace_path.to_path_buf());
    workspace_id_for_path(&normalized)
}

/// 根据工作区路径生成稳定 ID。
///
/// 参数:
/// - `workspace_path`: 工作区目录
///
/// 返回:
/// - 工作区 ID
pub fn workspace_id_for_path(workspace_path: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace_path.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("{:x}", digest)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: PathBuf) -> SaiPaths {
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

    #[test]
    fn workspace_ids_are_stable_and_path_specific() {
        let first = workspace_id_for_path(Path::new("/tmp/project-a"));
        let second = workspace_id_for_path(Path::new("/tmp/project-a"));
        let third = workspace_id_for_path(Path::new("/tmp/project-b"));

        assert_eq!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn workspace_scope_uses_workspace_directory() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let scope = workspace_scope_for_path(&paths, Path::new("/tmp/project-a"));

        assert!(scope.state_dir.starts_with(paths.state_dir));
        assert!(scope
            .state_dir
            .ends_with(workspace_id_for_path(Path::new("/tmp/project-a"))));
    }

    #[tokio::test]
    async fn locate_session_dirs_finds_other_workspace_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let cwd = temp.path().join("workspace-a");
        let other = temp.path().join("workspace-b");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        crate::runtime_cwd::scope(cwd, async {
            let foreign =
                crate::state::create_session_for_workspace(&paths, &other, Some("foreign"))
                    .unwrap();
            let local = crate::state::create_session(&paths, Some("local")).unwrap();

            let found = crate::state::locate_session_dirs(&paths, &foreign.id).unwrap();
            assert!(found
                .1
                .ends_with(std::path::Path::new("data").join(&foreign.id)));
            let local_found = crate::state::locate_session_dirs(&paths, &local.id).unwrap();
            assert!(local_found
                .1
                .ends_with(std::path::Path::new("data").join(&local.id)));
        })
        .await;
    }
}
