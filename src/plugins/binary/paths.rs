use crate::plugins::system::paths::{expand, open_anchor, resolve_existing_ancestor, workdir};
use anyhow::{bail, Context, Result};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use sai_plugin_runtime::{host::SystemContext, Capabilities};
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// 【插件写入】【目录句柄】目标父目录与文件名分开，发布不再依赖可变的绝对路径。
pub(super) struct Destination {
    pub directory: Dir,
    pub name: OsString,
    pub display: PathBuf,
}

/// 【插件写入】【路径授权】先解析目标及授权目录，再创建必要目录，绝不预先创建越界路径。
/// @param value 目标路径；context 为可信目录和权限；capabilities 为有效写入授权
/// @returns 无越界链接的父目录句柄及目标文件名
pub(super) fn authorize(
    value: &str,
    context: &SystemContext,
    capabilities: &Capabilities,
) -> Result<Destination> {
    capabilities
        .binary
        .check_write(value, context.allow_writes)?;
    let cwd = workdir(context)?;
    let display = expand(value, &cwd)?;
    validate_components(&display)?;
    let requested = resolve_existing_ancestor(&display)?;
    for declared in &capabilities.binary.write_paths {
        let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
        let Ok(relative) = requested.strip_prefix(&root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let name = relative
            .file_name()
            .context("plugin binary output needs a filename")?
            .to_os_string();
        // 1. 【插件写入】【授权后创建】完整归属已经确定，后续逐层拒绝被替换成链接的目录
        let mut directory = create_anchor(&root)?;
        if let Some(parent) = relative.parent() {
            for component in parent.components() {
                let Component::Normal(name) = component else {
                    bail!("invalid binary output directory");
                };
                directory = child(&directory, name)?;
            }
        }
        match directory.symlink_metadata(&name) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                bail!("plugin binary output must be a regular file")
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect plugin binary destination"),
        }
        return Ok(Destination {
            directory,
            name,
            display,
        });
    }
    bail!("plugin binary output is outside the granted write paths")
}

/// 【插件写入】【新目录锚定】从最近存在的规范祖先打开句柄，按已授权路径创建缺失目录。
/// @param path 已规范化的授权根
/// @returns 实际根目录句柄
fn create_anchor(path: &Path) -> Result<Dir> {
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

/// 【插件写入】【逐层创建】已经存在的路径也必须重新以不跟随链接的方式打开。
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

/// 【插件写入】【跨平台路径】禁止数据流名称、设备别名及尾部规范化歧义。
/// @param path 绝对输出路径
/// @returns 所有普通路径分量都能作为普通文件名时成功
fn validate_components(path: &Path) -> Result<()> {
    for component in path.components() {
        if let Component::Normal(name) = component {
            let name = name.to_string_lossy();
            let base = name
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if name.contains(':')
                || name.ends_with(['.', ' '])
                || matches!(
                    base.as_str(),
                    "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
                )
                || base
                    .strip_prefix("com")
                    .or_else(|| base.strip_prefix("lpt"))
                    .is_some_and(|suffix| {
                        matches!(
                            suffix,
                            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                        )
                    })
            {
                bail!("plugin binary output has a non-portable path component");
            }
        }
    }
    Ok(())
}
