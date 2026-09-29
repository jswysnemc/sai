use super::workspace::{workspace_scope_for_path, WorkspaceScope};
use crate::paths::SaiPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
struct WorkspaceMetadata {
    path: PathBuf,
}

/// 【会话工作区】【路径登记】在会话创建或打开时记录来源目录，不改变只读列表行为。
/// 参数: scope 为已定位工作区；返回: 保存结果
pub(super) fn record(scope: &WorkspaceScope) -> Result<()> {
    let file = scope.state_dir.join("workspace.json");
    let bytes = serde_json::to_vec(&WorkspaceMetadata {
        path: scope.workspace_path.clone(),
    })?;
    if std::fs::read(&file).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(&scope.state_dir)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&scope.state_dir)?;
        temporary.write_all(&bytes)?;
        temporary.persist(file)?;
    }
    Ok(())
}

/// 【会话工作区】【路径解析】读取来源路径，兼容已登记的 Web 工作区。
/// 参数: paths 为应用路径，id 为工作区标识，current 为当前目录；返回: 通过身份校验的路径
pub(super) fn resolve(paths: &SaiPaths, id: &str, current: &Path) -> Option<PathBuf> {
    let matches = |path: &Path| {
        workspace_scope_for_path(paths, path)
            .state_dir
            .file_name()
            .is_some_and(|name| name == id)
    };
    if matches(current) {
        return Some(current.to_path_buf());
    }
    let file = paths
        .state_dir
        .join("sessions/workspaces")
        .join(id)
        .join("workspace.json");
    if let Some(metadata) = std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WorkspaceMetadata>(&bytes).ok())
    {
        if metadata.path.is_absolute() && matches(&metadata.path) {
            return Some(metadata.path);
        }
    }
    let registry: serde_json::Value =
        serde_json::from_slice(&std::fs::read(paths.state_dir.join("web/workspaces.json")).ok()?)
            .ok()?;
    registry
        .get("workspaces")?
        .as_array()?
        .iter()
        .filter_map(|item| item.get("path")?.as_str())
        .map(PathBuf::from)
        .find(|path| path.is_absolute() && matches(path))
}
