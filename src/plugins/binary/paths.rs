use crate::plugins::file_ops::paths::validate_components;
use crate::plugins::system::paths::{expand, open_anchor, resolve_existing_ancestor, workdir};
use anyhow::{bail, Context, Result};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use sai_plugin_runtime::{host::SystemContext, Capabilities};
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// 【插件写入】【目录句柄】目标父目录与文件名分开，发布不再依赖可变的绝对路径
pub(super) struct Destination {
    pub directory: Dir,
    pub name: OsString,
    pub display: PathBuf,
}

/// 【插件写入】【授权计划】完整归属验证不创建输出目录，条件冲突可以直接返回
pub(super) struct Plan {
    pub canonical: PathBuf,
    display: PathBuf,
}

/// 【插件写入】【路径授权】只解析并验证目标归属，不创建目录或文件
/// @param value 目标路径；context 为可信目录和权限；capabilities 为有效写入授权
/// @returns 经过规范路径与目录范围校验的输出计划
pub(super) fn plan(
    value: &str,
    context: &SystemContext,
    capabilities: &Capabilities,
) -> Result<Plan> {
    // 1. 【插件写入】【规范目标】先检查请求格式并解析现有祖先，不创建任何输出目录
    capabilities
        .binary
        .check_write(value, context.allow_writes)?;
    let cwd = workdir(context)?;
    let display = expand(value, &cwd)?;
    validate_components(&display)?;
    let requested = resolve_existing_ancestor(&display)?;
    // 2. 【插件写入】【目录范围】完整目标必须严格位于某个有效输出目录内部
    for declared in &capabilities.binary.write_paths {
        let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
        if requested != root && requested.starts_with(&root) {
            return Ok(Plan {
                canonical: requested,
                display,
            });
        }
    }
    bail!("plugin binary output is outside the granted write paths")
}

impl Plan {
    /// 【插件写入】【读取范围】独立检查完整目标的读取授权，缺失根目录仍须明确满足路径范围
    /// @param context 可信工作目录；capabilities 为有效读取授权
    /// @returns 目标同时属于读取范围时成功，不把任意 NotFound 视为授权
    pub(super) fn check_read(
        &self,
        context: &SystemContext,
        capabilities: &Capabilities,
    ) -> Result<()> {
        let cwd = workdir(context)?;
        // 1. 【插件写入】【缺失根授权】先确认规范目标属于声明根，再处理该根尚不存在的情况
        for declared in &capabilities.system.read_paths {
            let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
            if !self.canonical.starts_with(&root) {
                continue;
            }
            match std::fs::symlink_metadata(&root) {
                Ok(metadata)
                    if metadata.is_dir() || (metadata.is_file() && self.canonical == root) =>
                {
                    return Ok(())
                }
                Ok(_) => continue,
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error).context("inspect conditional binary read root"),
            }
        }
        bail!("plugin binary output is outside the granted read paths")
    }

    /// 【插件写入】【现有父目录】取得真实目录句柄，父目录缺失时不创建任何内容
    /// @returns 已存在的父目录与目标名称；缺失父目录为 None，其他错误明确返回
    pub(super) fn existing(&self) -> Result<Option<Destination>> {
        let parent = self
            .canonical
            .parent()
            .context("plugin binary output needs a parent directory")?;
        let directory = match open_anchor(parent) {
            Ok(directory) => directory,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == ErrorKind::NotFound) =>
            {
                return Ok(None)
            }
            Err(error) => return Err(error),
        };
        self.destination(directory).map(Some)
    }

    /// 【插件写入】【授权后创建】在实际准备发布时创建已经授权的缺失父目录
    /// @returns 不跟随校验后替换链接的目标目录句柄
    pub(super) fn create(&self) -> Result<Destination> {
        let parent = self
            .canonical
            .parent()
            .context("plugin binary output needs a parent directory")?;
        self.destination(create_anchor(parent)?)
    }

    /// 【插件写入】【目标复核】输出目标只能是普通文件或缺失名称
    /// @param directory 已经取得的可信父目录句柄
    /// @returns 保留原始显示路径的输出目标
    fn destination(&self, directory: Dir) -> Result<Destination> {
        let name = self
            .canonical
            .file_name()
            .context("plugin binary output needs a filename")?
            .to_os_string();
        match directory.symlink_metadata(&name) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                bail!("plugin binary output must be a regular file")
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect plugin binary destination"),
        }
        Ok(Destination {
            directory,
            name,
            display: self.display.clone(),
        })
    }
}

/// 【插件写入】【新目录锚定】从最近存在的规范祖先打开句柄，按已授权路径创建缺失目录
/// @param path 已规范化并完成授权的父目录
/// @returns 实际父目录句柄
pub(super) fn create_anchor(path: &Path) -> Result<Dir> {
    let mut current = path.to_path_buf();
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.is_symlink() {
                    bail!("plugin binary root must be a directory without links");
                }
                let mut directory = open_anchor(&current)?;
                for name in missing.iter().rev() {
                    directory = child(&directory, name)?;
                }
                return Ok(directory);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                missing.push(
                    current
                        .file_name()
                        .context("binary root has no existing ancestor")?
                        .to_os_string(),
                );
                if !current.pop() {
                    bail!("binary root has no existing ancestor");
                }
            }
            Err(error) => return Err(error).context("inspect plugin binary root"),
        }
    }
}

/// 【插件写入】【逐层创建】已经存在的路径也必须重新以不跟随链接的方式打开
/// @param directory 已授权父目录；name 为单个路径分量
/// @returns 子目录句柄
fn child(directory: &Dir, name: &std::ffi::OsStr) -> Result<Dir> {
    match directory.create_dir(name) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("create plugin binary output directory"),
    }
    directory
        .open_dir_nofollow(name)
        .context("open plugin binary directory without following links")
}
