use super::workspace::workspace_scope_for_path;
use super::{list_all_sessions, list_located_sessions_for_workspace, LocatedSession};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// 【会话恢复】【精确目标】同时携带会话和工作区身份，允许不同工作区存在同名会话。
#[derive(Debug, Clone)]
pub(crate) struct ResumeTarget {
    pub session: LocatedSession,
    pub workspace_path: Option<PathBuf>,
}

impl ResumeTarget {
    /// 【会话恢复】【目录验证】恢复前验证路径存在且仍对应原索引。
    /// 参数: paths 为应用路径；返回: 规范化的目标目录
    pub(crate) fn directory(&self, paths: &SaiPaths) -> Result<PathBuf> {
        let path = self
            .workspace_path
            .as_ref()
            .context("Workspace path unavailable; use sai resume <id> --workspace <path>")?;
        let path = crate::platform::windows_path::canonicalize(path)
            .with_context(|| format!("Workspace unavailable: {}", path.display()))?;
        if !path.is_dir() {
            bail!("Workspace is not a directory: {}", path.display());
        }
        let scope = workspace_scope_for_path(paths, &path);
        if scope
            .state_dir
            .file_name()
            .is_none_or(|id| id != self.session.workspace_id.as_str())
        {
            bail!("Workspace identity changed: {}", path.display());
        }
        Ok(path)
    }
}

/// 【会话恢复】【目录读取】读取当前或全部会话，不创建占位会话。
/// 参数: paths 为存储路径，current 为当前目录，all 控制全量视图；返回: 带路径的会话
pub(crate) fn resume_catalog(
    paths: &SaiPaths,
    current: &Path,
    all: bool,
) -> Result<Vec<ResumeTarget>> {
    let sessions = if all {
        list_all_sessions(paths)?
    } else {
        list_located_sessions_for_workspace(paths, current)?
    };
    let mut directories = BTreeMap::new();
    let mut targets = sessions
        .into_iter()
        .map(|session| {
            let workspace_path = directories
                .entry(session.workspace_id.clone())
                .or_insert_with(|| {
                    super::workspace_metadata::resolve(paths, &session.workspace_id, current)
                })
                .clone();
            ResumeTarget {
                session,
                workspace_path,
            }
        })
        .collect::<Vec<_>>();
    targets.sort_by(|a, b| {
        a.workspace_path
            .cmp(&b.workspace_path)
            .then_with(|| a.session.workspace_id.cmp(&b.session.workspace_id))
            .then_with(|| b.session.info.updated_at.cmp(&a.session.info.updated_at))
            .then_with(|| a.session.info.id.cmp(&b.session.info.id))
    });
    Ok(targets)
}

/// 【会话恢复】【标识解析】优先匹配当前工作区，跨区同名标识必须明确指定目录。
/// 参数: paths 为应用路径，id 为会话标识，workspace 为可选明确目录；返回: 唯一目标
pub(crate) fn resolve_resume_target(
    paths: &SaiPaths,
    id: &str,
    workspace: Option<&Path>,
) -> Result<ResumeTarget> {
    let current = crate::runtime_cwd::current_dir()?;
    let local = resume_catalog(paths, workspace.unwrap_or(&current), false)?;
    if let Some(target) = local
        .into_iter()
        .find(|item| item.session.info.id == id.trim())
    {
        return Ok(target);
    }
    if workspace.is_some() {
        bail!("Session not found in requested workspace: {id}");
    }
    let mut candidates = resume_catalog(paths, &current, true)?
        .into_iter()
        .filter(|item| item.session.info.id == id.trim());
    let target = candidates
        .next()
        .with_context(|| format!("Session not found: {id}"))?;
    if candidates.next().is_some() {
        bail!("Session ID is ambiguous; use --workspace <path>: {id}");
    }
    Ok(target)
}
