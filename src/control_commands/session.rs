use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};

/// 创建并切换到新会话。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `title`: 可选会话标题文本
///
/// 返回:
/// - 创建结果文本
pub fn create_new_session(paths: &SaiPaths, title: &str) -> Result<String> {
    let title = title.trim();
    let session = if title.is_empty() {
        crate::state::create_session(paths, None)?
    } else {
        crate::state::create_session(paths, Some(title))?
    };
    Ok(format!(
        "{}: {}  {}",
        t("created session", "已创建会话"),
        session.id,
        session.title
    ))
}

/// 切换到指定会话（resume）。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `session_id`: 目标会话 ID
///
/// 返回:
/// - 切换结果文本
pub fn resume_session(paths: &SaiPaths, session_id: &str) -> Result<String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        bail!("{}", t("session id is required", "需要提供会话 ID"));
    }
    let session = crate::state::switch_session(paths, session_id)?;
    Ok(format!(
        "{}: {}  {}",
        t("current session", "当前会话"),
        session.id,
        session.title
    ))
}

/// 列出可供 resume 选择的会话标签。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - `(会话 ID, 展示文本)` 列表，按 `list_sessions` 顺序
pub fn session_resume_choices(paths: &SaiPaths) -> Result<Vec<(String, String)>> {
    let active = crate::state::active_session(paths)?;
    let sessions = crate::state::list_sessions(paths)?;
    if sessions.is_empty() {
        bail!("{}", t("no sessions available", "没有可用会话"));
    }
    Ok(sessions
        .into_iter()
        .map(|session| {
            let marker = if session.id == active.id { "*" } else { " " };
            let label = format!(
                "{marker} {}  {}  {}",
                session.id, session.updated_at, session.title
            );
            (session.id, label)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// 构造测试用路径。
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
    fn resume_session_switches_active_session() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path().to_path_buf());
        let created = crate::state::create_session(&paths, Some("alpha")).unwrap();
        let _ = crate::state::create_session(&paths, Some("beta")).unwrap();
        let message = resume_session(&paths, &created.id).unwrap();
        assert!(message.contains(&created.id));
        let active = crate::state::active_session(&paths).unwrap();
        assert_eq!(active.id, created.id);
    }

    #[test]
    fn session_resume_choices_marks_active_session() {
        let dir = tempdir().unwrap();
        let paths = test_paths(dir.path().to_path_buf());
        let _ = crate::state::create_session(&paths, Some("work")).unwrap();
        let choices = session_resume_choices(&paths).unwrap();
        assert!(choices.iter().any(|(_, label)| label.starts_with('*')));
        assert!(choices.len() >= 2);
    }
}
