//! 通过已打开的父目录执行操作，拒绝符号链接、特殊文件及硬链接别名。

use super::model::MAX_FILE_BYTES;
use anyhow::{Context, Result, ensure};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    ffi::OsString,
    fs::{Metadata, Permissions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub(super) struct Target {
    pub path: PathBuf,
    parent: Option<Dir>,
}

pub(super) struct Snapshot {
    pub content: String,
    pub metadata: Metadata,
    identity: (u64, u64, u64),
}

pub(super) struct CreatedDirectory {
    parent: Dir,
    directory: Dir,
    name: OsString,
}

impl CreatedDirectory {
    /// 【文件补丁】【目录清理】通过原父目录句柄清理空目录，不跟随并发替换的祖先链接。
    /// 参数: 无；返回: 无，目录已替换或仍有内容时保留它。
    pub fn discard(&self) {
        let Ok(current) = self.parent.open_dir_nofollow(&self.name) else {
            return;
        };
        if same_directory(&self.directory, &current).unwrap_or(false) {
            let _ = self.parent.remove_dir(&self.name);
        }
    }
}

/// 【文件补丁】【路径边界】展开当前工作区路径，拒绝父目录跳转及非文件目标。
/// 参数: path 为协议路径；返回: 无父目录跳转的绝对路径。
pub(super) fn resolve(path: &str) -> Result<PathBuf> {
    // 1. 【文件补丁】【原始路径】Windows 逐字路径拼接会消除父级跳转，必须先校验协议输入
    let original = Path::new(path.trim());
    ensure!(
        !original.components().any(|part| part.as_os_str() == ".."),
        "patch paths must not contain parent-directory traversal: {}",
        original.display()
    );
    // 2. 【文件补丁】【展开路径】继续检查展开结果，拒绝工作目录或家目录引入父级跳转
    let path = crate::tools::fs_path::expand_path(path);
    ensure!(
        path.is_absolute() && !path.components().any(|part| part.as_os_str() == ".."),
        "patch paths must not contain parent-directory traversal: {}",
        path.display()
    );
    ensure!(
        path.file_name().is_some(),
        "patch target must be a file: {}",
        path.display()
    );
    #[cfg(windows)]
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => ensure!(
                !matches!(
                    prefix.kind(),
                    std::path::Prefix::DeviceNS(_) | std::path::Prefix::Verbatim(_)
                ),
                "device namespace paths are not supported"
            ),
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                let stem = name
                    .split('.')
                    .next()
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                ensure!(
                    !name.contains(':')
                        && !name.ends_with(['.', ' '])
                        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        && !((stem.starts_with("COM") || stem.starts_with("LPT"))
                            && stem.len() == 4
                            && stem.as_bytes()[3].is_ascii_digit()),
                    "ambiguous or reserved Windows patch path: {name}"
                );
            }
            _ => {}
        }
    }
    Ok(path.components().collect())
}

impl Target {
    /// 【文件补丁】【目录固定】逐级打开父目录，禁止通过任何符号链接访问目标。
    /// 参数: path 为绝对路径；返回: 只读定位结果，缺少目录时暂不创建。
    pub fn new(path: PathBuf) -> Result<Self> {
        let parent = open_parent(&path, false, &mut Vec::new())?;
        Ok(Self { path, parent })
    }

    /// 【文件补丁】【快照读取】有界读取普通 UTF-8 文件，读取过程中发生变化则拒绝。
    /// 参数: 无；返回: 内容与元数据，不存在的文件返回空。
    pub fn snapshot(&self) -> Result<Option<Snapshot>> {
        let Some(parent) = &self.parent else {
            return Ok(None);
        };
        let name = self.path.file_name().unwrap();
        let metadata = match parent.symlink_metadata(name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "patch target is not a regular non-symlink file: {}",
            self.path.display()
        );
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            // 1. 【文件补丁】【特殊文件竞争】检查后若文件被换成 FIFO，非阻塞打开避免无限等待
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = parent.open_with(name, &options)?.into_std();
        let before = file.metadata()?;
        ensure!(
            before.is_file(),
            "patch target changed to a non-regular file: {}",
            self.path.display()
        );
        let identity = file_identity(&file)?;
        ensure!(
            identity.2 == 1,
            "patch refuses hard-linked files: {}",
            self.path.display()
        );
        ensure!(
            before.len() <= MAX_FILE_BYTES as u64,
            "file exceeds the 8 MiB limit: {}",
            self.path.display()
        );
        let mut bytes = Vec::new();
        (&file)
            .take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= MAX_FILE_BYTES && same_metadata(&before, &file.metadata()?),
            "file changed while reading: {}",
            self.path.display()
        );
        ensure!(
            !bytes.contains(&0),
            "patch refuses binary files: {}",
            self.path.display()
        );
        let content = String::from_utf8(bytes)
            .with_context(|| format!("patch requires UTF-8: {}", self.path.display()))?;
        ensure!(
            content.lines().count() <= 200_000,
            "file exceeds the 200000-line limit: {}",
            self.path.display()
        );
        Ok(Some(Snapshot {
            content,
            metadata: before,
            identity,
        }))
    }

    /// 【文件补丁】【冲突检查】重新定位父目录并比对内容、inode、修改时间和权限。
    /// 参数: expected 为准备阶段快照；返回: 无外部修改时成功。
    pub fn verify(&self, expected: Option<&Snapshot>) -> Result<()> {
        let fresh = Self::new(self.path.clone())?;
        let current = fresh.snapshot()?;
        ensure!(
            match (expected, current.as_ref()) {
                (None, None) => true,
                (Some(old), Some(new)) =>
                    old.content == new.content
                        && old.identity == new.identity
                        && same_metadata(&old.metadata, &new.metadata),
                _ => false,
            },
            "file edit conflict: {} changed before commit; read it again",
            self.path.display()
        );
        if let (Some(old), Some(new)) = (&self.parent, &fresh.parent) {
            ensure!(
                same_directory(old, new)?,
                "file edit conflict: parent directory changed: {}",
                self.path.display()
            );
        }
        Ok(())
    }

    /// 【文件补丁】【目录准备】全部补丁校验完成后才创建目标目录，并记录清理范围。
    /// 参数: created 为新建目录列表；返回: 父目录已固定时成功。
    pub fn ensure_parent(&mut self, created: &mut Vec<CreatedDirectory>) -> Result<()> {
        if self.parent.is_none() {
            self.parent = open_parent(&self.path, true, created)?;
        }
        Ok(())
    }

    /// 【文件补丁】【同目录暂存】独占创建临时文件，写入完整内容并保留原权限。
    /// 参数: content 为完整文本，permissions 为原文件权限；返回: 临时文件名。
    pub fn stage(&self, content: &str, permissions: Option<Permissions>) -> Result<String> {
        let name = format!(".sai-patch-{}.tmp", uuid::Uuid::new_v4());
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let parent = self
            .parent
            .as_ref()
            .context("missing prepared parent directory")?;
        let mut file = parent.open_with(&name, &options)?.into_std();
        let result = (|| {
            file.write_all(content.as_bytes())?;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions)?;
            }
            file.sync_all()
        })();
        if let Err(error) = result {
            let _ = parent.remove_file(&name);
            return Err(error.into());
        }
        Ok(name)
    }

    /// 【文件补丁】【原子替换】通过固定父目录提交暂存文件或删除源文件。
    /// 参数: staged 为临时文件名，空值表示删除；返回: 单文件提交结果。
    pub fn commit(&self, staged: Option<&str>) -> Result<()> {
        let parent = self
            .parent
            .as_ref()
            .context("missing target parent directory")?;
        let name = self.path.file_name().unwrap();
        if let Some(staged) = staged {
            parent.rename(staged, parent, name)?;
        } else {
            parent.remove_file(name)?;
        }
        Ok(())
    }

    /// 【文件补丁】【暂存清理】只移除本次调用创建的临时文件，不触及目标内容。
    /// 参数: name 为临时文件名；返回: 无，清理失败不覆盖原错误。
    pub fn discard(&self, name: &str) {
        if let Some(parent) = &self.parent {
            let _ = parent.remove_file(name);
        }
    }
}

/// 【文件补丁】【目录解析】逐级使用不跟随链接的目录句柄，避免检查后改写链接目标。
/// 参数: path 为文件路径，create 决定是否补齐目录，created 记录新目录；返回: 固定父目录。
fn open_parent(
    path: &Path,
    create: bool,
    created: &mut Vec<CreatedDirectory>,
) -> Result<Option<Dir>> {
    let parent = path.parent().context("file has no parent")?;
    let mut root = PathBuf::new();
    let mut components = parent.components().peekable();
    while components
        .peek()
        .is_some_and(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        root.push(components.next().unwrap().as_os_str());
    }
    let mut dir = Dir::open_ambient_dir(&root, cap_std::ambient_authority())?;
    for component in components {
        if component == Component::CurDir {
            continue;
        }
        ensure!(
            matches!(component, Component::Normal(_)),
            "invalid patch path"
        );
        root.push(component.as_os_str());
        let next = match dir.open_dir_nofollow(component.as_os_str()) {
            Ok(next) => next,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                let made = match dir.create_dir(component.as_os_str()) {
                    Ok(()) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
                    Err(error) => return Err(error.into()),
                };
                let next = dir.open_dir_nofollow(component.as_os_str())?;
                if made {
                    created.push(CreatedDirectory {
                        parent: dir.try_clone()?,
                        directory: next.try_clone()?,
                        name: component.as_os_str().to_os_string(),
                    });
                }
                next
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("patch parent must not be a symlink: {}", root.display())
                });
            }
        };
        dir = next;
    }
    Ok(Some(dir))
}

/// 【文件补丁】【元数据比较】识别同内容替换和权限变化，防止旧快照覆盖新文件。
/// 参数: left/right 为两次元数据；返回: 文件版本是否相同。
fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (
            left.dev(),
            left.ino(),
            left.mode(),
            left.ctime(),
            left.ctime_nsec(),
        ) != (
            right.dev(),
            right.ino(),
            right.mode(),
            right.ctime(),
            right.ctime_nsec(),
        ) {
            return false;
        }
    }
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.permissions().readonly() == right.permissions().readonly()
}

/// 【文件补丁】【目录身份】比较固定目录与当前路径，拒绝替换父目录后的提交。
/// 参数: left/right 为目录句柄；返回: 是否指向同一目录。
fn same_directory(left: &Dir, right: &Dir) -> Result<bool> {
    let left = file_identity(&left.try_clone()?.into_std_file())?;
    let right = file_identity(&right.try_clone()?.into_std_file())?;
    Ok(left.0 == right.0 && left.1 == right.1)
}

/// 【文件补丁】【跨平台身份】读取文件卷、文件标识和链接数，识别路径别名与同内容替换。
/// 参数: file 为已打开句柄；返回: 卷标识、文件标识及硬链接数量。
fn file_identity(file: &std::fs::File) -> Result<(u64, u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        Ok((metadata.dev(), metadata.ino(), metadata.nlink()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // 1. 【文件补丁】【句柄查询】结构由系统完整写入，调用期间文件句柄保持有效
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok((
            u64::from(information.dwVolumeSerialNumber),
            (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
            u64::from(information.nNumberOfLinks),
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        anyhow::bail!("safe patch file identity is unsupported on this platform")
    }
}
