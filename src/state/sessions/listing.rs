use super::index::{
    ensure_default_session_for_base, read_current_session_id_from_base,
    write_current_session_id_to_base,
};
use super::model::{LocatedSession, DEFAULT_SESSION_ID};
use super::repository::{
    current_session_scope, migrate_legacy_sessions_to_workspace, session_state_dir,
};
use super::workspace::{workspace_scope_for_path, WorkspaceScope};
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::path::Path;

/// 【会话载入】【当前工作区】一次读取索引，并为每个会话附上已知目录。
/// @param paths 应用路径集合
/// @returns 当前工作区按更新时间排序的会话及活动指针
pub fn list_located_sessions(paths: &SaiPaths) -> Result<Vec<LocatedSession>> {
    list_scope(&current_session_scope(paths)?)
}

/// 【会话载入】【指定工作区】一次读取指定工作区索引，避免逐行重新定位会话。
/// @param paths 应用路径集合；workspace_path 为工作区目录
/// @returns 按更新时间排序的会话、所属目录和活动指针
pub fn list_located_sessions_for_workspace(
    paths: &SaiPaths,
    workspace_path: &Path,
) -> Result<Vec<LocatedSession>> {
    let scope = workspace_scope_for_path(paths, workspace_path);
    migrate_legacy_sessions_to_workspace(paths, &scope.state_dir)?;
    list_scope(&scope)
}

/// 【会话载入】【索引快照】从同一次索引读取生成展示数据，不创建各会话的数据目录。
/// @param scope 已定位的工作区会话作用域
/// @returns 元数据与目录保持一致的会话列表
fn list_scope(scope: &WorkspaceScope) -> Result<Vec<LocatedSession>> {
    let sessions = ensure_default_session_for_base(&scope.state_dir)?;
    let mut current = read_current_session_id_from_base(&scope.state_dir)?;
    // 1. 【会话载入】【活动指针】只修复已失效的选择，正常读取不改写任何索引
    if !sessions.iter().any(|session| session.id == current) {
        current = DEFAULT_SESSION_ID.to_string();
        write_current_session_id_to_base(&scope.state_dir, &current)?;
    }
    let workspace_id = scope
        .state_dir
        .file_name()
        .context("workspace session scope is missing its identifier")?
        .to_string_lossy()
        .into_owned();
    // 2. 【会话载入】【目录绑定】索引已验证会话身份，展示持有者时直接使用对应目录
    Ok(sessions
        .into_iter()
        .map(|info| LocatedSession {
            state_dir: session_state_dir(&scope.state_dir, &info.id),
            is_current: info.id == current,
            workspace_id: workspace_id.clone(),
            info,
        })
        .collect())
}

#[cfg(test)]
mod tests;
