use super::super::{cancel::check_cancelled, target::Target};
use crate::plugins::system::paths::{open_anchor, resolve_existing_ancestor};
use anyhow::{bail, Context, Result};
use cap_fs_ext::DirExt;
use cap_std::fs::{Dir, DirBuilder, DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    ffi::{OsStr, OsString},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// 【插件回收站】【专用目录】原文件和还原信息分别写入标准目录，保留独立能力句柄
pub(super) struct Directories {
    root: Dir,
    pub files: Dir,
    pub info: Dir,
    pub canonical: PathBuf,
}

/// 【插件回收站】【用户目录】仅使用可信宿主环境选择 XDG 数据目录，Lua 无法指定回收站位置
/// @returns 当前用户数据目录，缺失或非绝对目录明确拒绝
pub(super) fn home() -> Result<PathBuf> {
    let base =
        ::directories::BaseDirs::new().context("plugin trash data directory is unavailable")?;
    let path = base.data_dir().to_path_buf();
    if !path.is_absolute() {
        bail!("plugin trash data directory must be absolute");
    }
    Ok(path)
}

/// 【插件回收站】【目录准备】创建或验证当前用户的私有回收站，只允许同一文件系统移动
/// @param data_home 可信 XDG 数据目录；target 为授权文件；cancelled 为撤销标记
/// @returns 三个通过所有权、权限和文件系统检查的目录信息
pub(super) fn open(
    data_home: &Path,
    target: &Target,
    cancelled: &AtomicBool,
) -> Result<Directories> {
    // 1. 【插件回收站】【固定位置】只解析数据根，回收站本身和两个子目录都不得是链接
    check_cancelled(cancelled)?;
    if !data_home.is_absolute() {
        bail!("plugin trash data directory must be absolute");
    }
    let data_home = resolve_existing_ancestor(data_home)?;
    let canonical = data_home.join("Trash");
    if target.canonical.starts_with(&canonical) {
        bail!("plugin cannot trash files already inside the system trash");
    }
    let data = create_data_home(&data_home, cancelled)?;
    if data.dir_metadata()?.dev() != target.metadata.dev() {
        bail!("plugin trash requires the same filesystem as the user trash");
    }
    // 2. 【插件回收站】【私有目录】不修改已有目录的权限，不跟随链接，不跨设备复制文件
    let trash = private_child(&data, OsStr::new("Trash"), cancelled)?;
    let files = private_child(&trash, OsStr::new("files"), cancelled)?;
    let info = private_child(&trash, OsStr::new("info"), cancelled)?;
    for directory in [&trash, &files, &info] {
        if directory.dir_metadata()?.dev() != target.metadata.dev() {
            bail!("plugin trash requires the same filesystem as the user trash");
        }
    }
    check_cancelled(cancelled)?;
    Ok(Directories {
        root: trash,
        files,
        info,
        canonical,
    })
}

impl Directories {
    /// 【插件回收站】【标准位置复核】确认根目录、文件目录与信息目录仍然对应准备时的句柄
    /// @returns 名称、所有权和私有权限保持一致时成功
    pub(super) fn check_current(&self) -> Result<()> {
        let root =
            open_anchor(&self.canonical).context("plugin trash directory changed before commit")?;
        let files = root
            .open_dir_nofollow("files")
            .context("plugin trash files directory changed before commit")?;
        let info = root
            .open_dir_nofollow("info")
            .context("plugin trash info directory changed before commit")?;
        for (current, expected) in [
            (&root, &self.root),
            (&files, &self.files),
            (&info, &self.info),
        ] {
            validate_private(current)?;
            let current = current.dir_metadata()?;
            let expected = expected.dir_metadata()?;
            if (current.dev(), current.ino()) != (expected.dev(), expected.ino()) {
                bail!("plugin trash directory changed before commit");
            }
        }
        Ok(())
    }
}

/// 【插件回收站】【目录权限】原子创建 0700 子目录，再通过句柄核对当前用户所有权
/// @param parent 已固定父目录；name 为单个名称；cancelled 为取消标记
/// @returns 当前用户独占的目录句柄
fn private_child(parent: &Dir, name: &OsStr, cancelled: &AtomicBool) -> Result<Dir> {
    let directory = child(parent, name, cancelled)?;
    validate_private(&directory)?;
    Ok(directory)
}

/// 【插件回收站】【私有属性】检查已经打开的目录属于当前用户且具有 0700 权限
/// @param directory 待检查目录句柄
/// @returns 私有属性有效时成功
fn validate_private(directory: &Dir) -> Result<()> {
    let metadata = directory.dir_metadata()?;
    // 1. 【插件回收站】【身份检查】geteuid 不读取外部数据，目录属性来自已打开句柄
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid || metadata.permissions().mode() & 0o7777 != 0o700 {
        bail!("plugin trash directory must be owned by the current user with mode 0700");
    }
    Ok(())
}

/// 【插件回收站】【逐层创建】缺失目录使用私有权限创建，已有目录仍以不跟随链接方式打开
/// @param parent 父目录；name 为单个分量；cancelled 为撤销标记
/// @returns 子目录句柄
fn child(parent: &Dir, name: &OsStr, cancelled: &AtomicBool) -> Result<Dir> {
    check_cancelled(cancelled)?;
    match parent.create_dir_with(name, DirBuilder::new().mode(0o700)) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("create plugin trash directory"),
    }
    parent
        .open_dir_nofollow(name)
        .context("open plugin trash directory without links")
}

/// 【插件回收站】【数据根补全】从最近存在的规范祖先逐层创建可信数据目录
/// @param path 规范绝对路径；cancelled 为取消标记
/// @returns 数据根目录句柄；不跟随解析完成后替换的链接
fn create_data_home(path: &Path, cancelled: &AtomicBool) -> Result<Dir> {
    let mut current = path.to_path_buf();
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        check_cancelled(cancelled)?;
        match open_anchor(&current) {
            Ok(mut directory) => {
                for name in missing.iter().rev() {
                    directory = child(&directory, name, cancelled)?;
                }
                return Ok(directory);
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == ErrorKind::NotFound) =>
            {
                missing.push(
                    current
                        .file_name()
                        .context("plugin trash root has no existing ancestor")?
                        .to_os_string(),
                );
                if !current.pop() {
                    bail!("plugin trash root has no existing ancestor");
                }
            }
            Err(error) => return Err(error),
        }
    }
}
