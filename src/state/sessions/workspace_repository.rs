use super::model::SessionInfo;
use super::repository::{
    current_session_scope, ensure_default_session_for_base, migrate_legacy_sessions_to_workspace,
    read_current_session_id_from_base, sanitize_session_id, save_sessions_to_base,
    session_state_dir, sort_sessions,
};
use super::workspace::workspace_scope_for_path;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

/// 读取当前工作区的会话列表。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 当前工作区会话列表
pub fn list_sessions(paths: &SaiPaths) -> Result<Vec<SessionInfo>> {
    let scope = current_session_scope(paths)?;
    ensure_default_session_for_base(&scope.state_dir)
}

/// 读取指定工作区的会话列表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
///
/// 返回:
/// - 指定工作区的会话列表
pub fn list_sessions_for_workspace(
    paths: &SaiPaths,
    workspace_path: &Path,
) -> Result<Vec<SessionInfo>> {
    let scope = workspace_scope_for_path(paths, workspace_path);
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    ensure_default_session_for_base(&scope.state_dir)
}

/// 读取指定工作区的当前会话标识。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
///
/// 返回:
/// - 当前会话标识
pub fn active_session_id_for_workspace(paths: &SaiPaths, workspace_path: &Path) -> Result<String> {
    let scope = workspace_scope_for_path(paths, workspace_path);
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    ensure_default_session_for_base(&scope.state_dir)?;
    read_current_session_id_from_base(&scope.state_dir)
}

/// 在指定工作区中确保稳定标识的会话存在。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `session_id`: 稳定会话标识
/// - `title`: 会话标题
///
/// 返回:
/// - 已存在或新创建的会话信息
pub fn ensure_workspace_session(
    paths: &SaiPaths,
    workspace_path: &Path,
    session_id: &str,
    title: &str,
) -> Result<SessionInfo> {
    let session_id = sanitize_session_id(session_id);
    let title = title.trim();
    if title.is_empty() {
        bail!("session title cannot be empty");
    }
    let scope = workspace_scope_for_path(paths, workspace_path);
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    let mut sessions = ensure_default_session_for_base(&scope.state_dir)?;
    if let Some(session) = sessions.iter_mut().find(|session| session.id == session_id) {
        if session.title != title {
            session.title = title.to_string();
            session.updated_at = Utc::now().to_rfc3339();
            let updated = session.clone();
            sort_sessions(&mut sessions);
            save_sessions_to_base(&scope.state_dir, &sessions)?;
            return Ok(updated);
        }
        return Ok(session.clone());
    }
    let now = Utc::now().to_rfc3339();
    let session = SessionInfo {
        id: session_id,
        title: title.to_string(),
        created_at: now.clone(),
        updated_at: now,
    };
    sessions.push(session.clone());
    sort_sessions(&mut sessions);
    save_sessions_to_base(&scope.state_dir, &sessions)?;
    std::fs::create_dir_all(session_state_dir(&scope.state_dir, &session.id))?;
    Ok(session)
}

/// 返回指定工作区和会话的状态目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 工作区状态目录与指定会话状态目录
pub fn state_dir_for_workspace_session(
    paths: &SaiPaths,
    workspace_path: &Path,
    session_id: &str,
) -> Result<(PathBuf, PathBuf)> {
    let scope = workspace_scope_for_path(paths, workspace_path);
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    let session_id = session_id.trim();
    ensure_default_session_for_base(&scope.state_dir)?
        .into_iter()
        .find(|session| session.id == session_id)
        .with_context(|| format!("session not found: {session_id}"))?;
    let state_dir = session_state_dir(&scope.state_dir, session_id);
    std::fs::create_dir_all(&state_dir)?;
    Ok((scope.state_dir, state_dir))
}
