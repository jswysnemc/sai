use super::{archive, paths};
use crate::{paths::SaiPaths, plugins::system};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use sai_plugin_runtime::{host::*, Capabilities};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// 【插件目录】【缓存租约】缓存内容保留供审查，锁的寿命覆盖目录中的全部进程和解压任务。
struct Workspace {
    root: PathBuf,
    directory: Arc<Dir>,
    lock: Arc<paths::Lock>,
}

/// 【插件目录】【创建】同一个插件、会话和键串行重建缓存，不解释模型提供的磁盘路径。
/// @param paths 应用路径；id 为插件；session 为会话；key 为目录键；capabilities 为授权
/// @returns 持锁的私有工作目录
pub(super) fn open(
    paths: &SaiPaths,
    id: &str,
    session: &str,
    key: &str,
    capabilities: &Capabilities,
) -> Result<Arc<dyn PluginWorkspace>> {
    if !capabilities.system.workspace {
        bail!("plugin private workspace is not allowed");
    }
    validate_storage_key(key)?;
    let (directory, display) =
        paths::namespace(&paths.cache_dir, "plugin-workspaces", id, session)?;
    let name = paths::hash(key);
    let _index = paths::lock(&directory, ".index.lock")?;
    let exists = directory.try_exists(&name)?;
    if !exists && directory.entries()?.take(257).count() >= 257 {
        bail!("plugin private workspace cache exceeds 128 keys");
    }
    let lock = paths::lock(&directory, &format!(".{name}.lock"))?;
    if exists {
        directory
            .open_dir_nofollow(&name)
            .context("private workspace must not be a symbolic link")?;
        directory.remove_dir_all(&name)?;
    }
    directory.create_dir(&name)?;
    Ok(Arc::new(Workspace {
        root: display.join(&name),
        directory: Arc::new(directory.open_dir_nofollow(&name)?),
        lock: Arc::new(lock),
    }))
}

impl Workspace {
    /// 【插件目录】【读取范围】仅为当前目录构造内部读取授权，不交给 Lua 扩大范围。
    /// @param path 规范相对路径
    /// @returns 绝对目标、可信上下文与当前目录授权
    fn read_context(&self, path: &str) -> Result<(String, SystemContext, Capabilities)> {
        validate_workspace_path(path)?;
        let mut capabilities = Capabilities::default();
        capabilities
            .system
            .read_paths
            .insert(self.root.to_string_lossy().into_owned());
        Ok((
            self.root.join(path).to_string_lossy().into_owned(),
            SystemContext {
                workdir: self.path(),
                allow_writes: false,
            },
            capabilities,
        ))
    }
}

#[async_trait]
impl PluginWorkspace for Workspace {
    /// 【插件目录】【显示】返回缓存目录的绝对路径。
    /// @returns 显示路径
    fn path(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    /// 【插件目录】【正文】复用有界读取与句柄授权。
    /// @param request 相对路径及读取限制
    /// @returns 文本结果
    async fn read_text(&self, mut request: FileReadRequest) -> Result<FileText> {
        let (path, context, capabilities) = self.read_context(&request.path)?;
        request.path = path;
        system::read_text(request, context, capabilities).await
    }

    /// 【插件目录】【列表】返回相对目录条目，不泄漏其他授权根。
    /// @param path 相对目录；max_entries 为条数上限
    /// @returns 目录列表
    async fn read_directory(&self, path: String, max_entries: usize) -> Result<DirectoryListing> {
        let (path, context, capabilities) = self.read_context(&path)?;
        let mut result = system::read_directory(path, max_entries, context, capabilities).await?;
        for entry in &mut result.entries {
            entry.path = Path::new(&entry.path)
                .strip_prefix(&self.root)?
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
        }
        Ok(result)
    }

    /// 【插件目录】【属性】越过目录的链接不视为缺失文件。
    /// @param path 相对路径
    /// @returns 可选文件属性
    async fn file_info(&self, path: String) -> Result<Option<FileInfo>> {
        let (path, context, capabilities) = self.read_context(&path)?;
        system::file_info(path, context, capabilities).await
    }

    /// 【插件目录】【解压】保留目录锁直到后台解压线程完成清理。
    /// @param request 下载与展开限制；capabilities 为有效授权
    /// @returns 完整发布结果
    async fn extract_archive(
        &self,
        request: ArchiveRequest,
        capabilities: Capabilities,
    ) -> Result<()> {
        archive::extract(
            self.directory.clone(),
            self.lock.clone(),
            request,
            capabilities,
        )
        .await
    }

    /// 【插件目录】【进程】只在真实路径仍属于当前目录时执行私有目录模板。
    /// @param request 模板；directory 为相对目录；capabilities 为授权；allow_writes 为工具权限
    /// @returns 进程结果
    async fn process(
        &self,
        request: ProcessRequest,
        directory: String,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<ProcessOutput> {
        validate_workspace_path(&directory)?;
        let path = dunce::canonicalize(self.root.join(directory))?;
        if !path.starts_with(&self.root) || !path.is_dir() {
            bail!("process directory is outside the private workspace");
        }
        system::execute_workspace(
            request,
            SystemContext {
                workdir: path.to_string_lossy().into_owned(),
                allow_writes,
            },
            capabilities,
        )
        .await
    }
}
