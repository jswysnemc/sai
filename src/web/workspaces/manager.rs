use super::model::{WorkspaceInfo, WorkspaceRegistry};
use crate::paths::SaiPaths;
use crate::state::workspace_id_for_path;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// 管理 Web 服务可切换的工作区集合。
#[derive(Clone)]
pub(crate) struct WorkspaceManager {
    registry_file: PathBuf,
    registry: Arc<RwLock<WorkspaceRegistry>>,
}

impl WorkspaceManager {
    /// 读取工作区注册表并确保当前目录已登记。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 工作区管理器
    pub(crate) fn new(paths: &SaiPaths) -> Result<Self> {
        let registry_file = paths.state_dir.join("web/workspaces.json");
        let current = canonical_directory(&std::env::current_dir()?)?;
        let gateway_workspace = crate::gateways::workspace::gateway_workspace_path(paths);
        std::fs::create_dir_all(&gateway_workspace)?;
        let gateway_workspace = canonical_directory(&gateway_workspace)?;
        let mut registry = load_registry(&registry_file)?;
        registry
            .workspaces
            .retain(|workspace| Path::new(&workspace.path).is_dir());
        let current_id = workspace_id_for_path(&current);
        if !registry
            .workspaces
            .iter()
            .any(|workspace| workspace.id == current_id)
        {
            registry.workspaces.push(workspace_info(&current, None));
        }
        let gateway_id = workspace_id_for_path(&gateway_workspace);
        if let Some(workspace) = registry
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == gateway_id)
        {
            workspace.name = "Gateway sessions".to_string();
        } else {
            registry
                .workspaces
                .push(workspace_info(&gateway_workspace, Some("Gateway sessions")));
        }
        if !registry
            .workspaces
            .iter()
            .any(|workspace| workspace.id == registry.active_id)
        {
            registry.active_id = current_id;
        }
        let manager = Self {
            registry_file,
            registry: Arc::new(RwLock::new(registry)),
        };
        let active = manager.active()?;
        std::env::set_current_dir(&active.path)
            .with_context(|| format!("failed to enter workspace {}", active.path))?;
        manager.save()?;
        Ok(manager)
    }

    /// 返回全部工作区。
    ///
    /// 返回:
    /// - 工作区列表
    pub(crate) fn list(&self) -> Result<Vec<WorkspaceInfo>> {
        Ok(self.read_registry()?.workspaces.clone())
    }

    /// 返回当前活动工作区。
    ///
    /// 返回:
    /// - 活动工作区
    pub(crate) fn active(&self) -> Result<WorkspaceInfo> {
        let registry = self.read_registry()?;
        registry
            .workspaces
            .iter()
            .find(|workspace| workspace.id == registry.active_id)
            .cloned()
            .context("active workspace is missing")
    }

    /// 返回指定工作区。
    ///
    /// 参数:
    /// - `id`: 工作区 ID
    ///
    /// 返回:
    /// - 工作区信息
    pub(crate) fn get(&self, id: &str) -> Result<WorkspaceInfo> {
        self.read_registry()?
            .workspaces
            .iter()
            .find(|workspace| workspace.id == id)
            .cloned()
            .with_context(|| format!("workspace not found: {id}"))
    }

    /// 添加工作区。
    ///
    /// 参数:
    /// - `path`: 工作区目录
    /// - `name`: 可选展示名称
    ///
    /// 返回:
    /// - 新增或已存在的工作区
    pub(crate) fn add(&self, path: &Path, name: Option<&str>) -> Result<WorkspaceInfo> {
        let path = canonical_directory(path)?;
        let id = workspace_id_for_path(&path);
        let mut registry = self.write_registry()?;
        if let Some(existing) = registry
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
        {
            if let Some(name) = normalized_name(name) {
                existing.name = name;
            }
            let result = existing.clone();
            drop(registry);
            self.save()?;
            return Ok(result);
        }
        let workspace = workspace_info(&path, name);
        registry.workspaces.push(workspace.clone());
        drop(registry);
        self.save()?;
        Ok(workspace)
    }

    /// 更新工作区名称。
    ///
    /// 参数:
    /// - `id`: 工作区 ID
    /// - `name`: 新名称
    ///
    /// 返回:
    /// - 更新后的工作区
    pub(crate) fn rename(&self, id: &str, name: &str) -> Result<WorkspaceInfo> {
        let name = normalized_name(Some(name)).context("workspace name cannot be empty")?;
        let mut registry = self.write_registry()?;
        let workspace = registry
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .with_context(|| format!("workspace not found: {id}"))?;
        workspace.name = name;
        let result = workspace.clone();
        drop(registry);
        self.save()?;
        Ok(result)
    }

    /// 切换活动工作区并更新进程当前目录。
    ///
    /// 参数:
    /// - `id`: 目标工作区 ID
    ///
    /// 返回:
    /// - 切换后的工作区
    pub(crate) fn switch(&self, id: &str) -> Result<WorkspaceInfo> {
        let mut registry = self.write_registry()?;
        let workspace = registry
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .with_context(|| format!("workspace not found: {id}"))?;
        let path = canonical_directory(Path::new(&workspace.path))?;
        std::env::set_current_dir(&path)
            .with_context(|| format!("failed to enter workspace {}", path.display()))?;
        workspace.path = path.display().to_string();
        workspace.last_opened_at = Utc::now().to_rfc3339();
        let result = workspace.clone();
        registry.active_id = result.id.clone();
        drop(registry);
        self.save()?;
        Ok(result)
    }

    /// 移除非活动工作区。
    ///
    /// 参数:
    /// - `id`: 工作区 ID
    ///
    /// 返回:
    /// - 是否完成移除
    pub(crate) fn remove(&self, id: &str) -> Result<bool> {
        let mut registry = self.write_registry()?;
        if registry.active_id == id {
            bail!("active workspace cannot be removed");
        }
        let before = registry.workspaces.len();
        registry.workspaces.retain(|workspace| workspace.id != id);
        let removed = registry.workspaces.len() != before;
        drop(registry);
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// 将注册表写入磁盘。
    fn save(&self) -> Result<()> {
        let registry = self.read_registry()?.clone();
        let parent = self
            .registry_file
            .parent()
            .context("workspace registry has no parent directory")?;
        std::fs::create_dir_all(parent)?;
        let temp = tempfile::NamedTempFile::new_in(parent)?;
        std::fs::write(temp.path(), serde_json::to_vec_pretty(&registry)?)?;
        temp.persist(&self.registry_file)?;
        Ok(())
    }

    /// 获取注册表读锁。
    fn read_registry(&self) -> Result<std::sync::RwLockReadGuard<'_, WorkspaceRegistry>> {
        self.registry
            .read()
            .map_err(|_| anyhow::anyhow!("workspace registry lock is poisoned"))
    }

    /// 获取注册表写锁。
    fn write_registry(&self) -> Result<std::sync::RwLockWriteGuard<'_, WorkspaceRegistry>> {
        self.registry
            .write()
            .map_err(|_| anyhow::anyhow!("workspace registry lock is poisoned"))
    }
}

/// 从磁盘读取工作区注册表。
fn load_registry(path: &Path) -> Result<WorkspaceRegistry> {
    if !path.exists() {
        return Ok(WorkspaceRegistry::default());
    }
    let raw = std::fs::read(path)?;
    serde_json::from_slice(&raw).context("invalid web workspace registry")
}

/// 规范化并校验工作区目录。
fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = crate::platform::windows_path::canonicalize(path)
        .with_context(|| format!("workspace does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("workspace is not a directory: {}", canonical.display());
    }
    Ok(crate::platform::windows_path::simplified(&canonical))
}

/// 构造工作区信息。
fn workspace_info(path: &Path, name: Option<&str>) -> WorkspaceInfo {
    let name = normalized_name(name).unwrap_or_else(|| {
        path.file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("workspace")
            .to_string()
    });
    WorkspaceInfo {
        id: workspace_id_for_path(path),
        name,
        path: path.display().to_string(),
        last_opened_at: Utc::now().to_rfc3339(),
    }
}

/// 规范化可选工作区名称。
fn normalized_name(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
