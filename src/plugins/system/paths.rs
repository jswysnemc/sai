use anyhow::{bail, Context, Result};
use cap_fs_ext::DirExt;
use cap_std::{ambient_authority, fs::Dir};
use sai_plugin_runtime::host::SystemContext;
use sai_plugin_runtime::Capabilities;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// 【插件系统】【路径句柄】以已经授权的目录句柄为根，后续操作不能通过符号链接离开该根。
pub(super) struct AuthorizedPath {
    pub directory: Dir,
    pub relative: PathBuf,
    pub display: PathBuf,
}

impl AuthorizedPath {
    /// 【插件系统】【目录打开】拒绝校验后替换的末级链接，单文件授权不能借父句柄枚举相邻目录。
    /// @returns 请求目录的独立能力句柄
    pub fn open_directory(&self) -> Result<Dir> {
        self.directory
            .open_dir_nofollow(&self.relative)
            .context("open plugin directory without following links")
    }
}

/// 【插件系统】【工作目录】使用宿主交付的绝对目录，库直接调用时使用宿主当前目录。
/// @param context 可信调用上下文
/// @returns 实际工作目录；显式相对目录属于无效宿主输入
pub(super) fn workdir(context: &SystemContext) -> Result<PathBuf> {
    if context.workdir.is_empty() {
        return std::env::current_dir().context("read host working directory");
    }
    let path = PathBuf::from(&context.workdir);
    if !path.is_absolute() {
        bail!("plugin host working directory must be absolute");
    }
    Ok(path)
}

/// 【插件系统】【路径授权】先比较解析后的真实路径，再从授权根目录句柄执行后续操作。
/// @param path 请求路径；context 为宿主目录；capabilities 为有效读取授权
/// @returns 不能越过授权根的句柄及相对路径
pub(super) fn authorize(
    path: &str,
    context: &SystemContext,
    capabilities: &Capabilities,
) -> Result<AuthorizedPath> {
    capabilities.system.check_read_request(path)?;
    let cwd = workdir(context)?;
    let display = expand(path, &cwd)?;
    let requested = resolve_existing_ancestor(&display)?;
    for declared in &capabilities.system.read_paths {
        let root = resolve_existing_ancestor(&expand(declared, &cwd)?)?;
        if !requested.starts_with(&root) {
            continue;
        }
        let metadata = match std::fs::metadata(&root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Err(error.into()),
            Err(error) => return Err(error).context("inspect authorized plugin path"),
        };
        let (anchor, relative) = if metadata.is_dir() {
            let relative = requested.strip_prefix(&root)?.to_path_buf();
            (
                root,
                if relative.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    relative
                },
            )
        } else if requested == root {
            let parent = root
                .parent()
                .context("authorized file has no parent")?
                .to_path_buf();
            let name = root.file_name().context("authorized file has no name")?;
            (parent, PathBuf::from(name))
        } else {
            continue;
        };
        let directory = open_anchor(&anchor)?;
        return Ok(AuthorizedPath {
            directory,
            relative,
            display,
        });
    }
    bail!("plugin file path is outside the granted read paths")
}

/// 【插件系统】【目录锚定】逐级打开规范路径，拒绝在路径校验后被替换成链接的目录。
/// @param path 已规范化的绝对目录
/// @returns 不再依赖可变路径名称的目录句柄
fn open_anchor(path: &Path) -> Result<Dir> {
    let mut root = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => root.push(component),
            Component::Normal(name) => names.push(name),
            _ => bail!("plugin directory anchor must be canonical"),
        }
    }
    if !root.is_absolute() {
        bail!("plugin directory anchor must be absolute");
    }
    let mut directory =
        Dir::open_ambient_dir(root, ambient_authority()).context("open plugin filesystem root")?;
    for name in names {
        directory = directory
            .open_dir_nofollow(name)
            .context("open authorized plugin directory without following links")?;
    }
    Ok(directory)
}

/// 【插件系统】【路径展开】展开当前用户目录或可信工作目录，不解释其他环境变量。
/// @param value 声明或请求路径；cwd 为绝对工作目录
/// @returns 绝对路径
fn expand(value: &str, cwd: &Path) -> Result<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        let user = directories::UserDirs::new().context("host user directory is unavailable")?;
        return Ok(user.home_dir().join(value.strip_prefix("~/").unwrap_or("")));
    }
    let path = Path::new(value);
    let expanded = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    if !expanded.is_absolute() {
        bail!("plugin path must resolve to an absolute path");
    }
    Ok(expanded)
}

#[cfg(all(test, unix))]
#[path = "tests/anchors.rs"]
mod tests;

/// 【插件系统】【缺失路径】解析最近存在的祖先，防止不存在的目标掩盖上层符号链接越界。
/// @param path 绝对路径
/// @returns 解析现有链接后重新附加缺失路径段的绝对路径
fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf> {
    let mut current = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match std::fs::canonicalize(&current) {
            Ok(mut resolved) => {
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let name = current
                    .file_name()
                    .context("plugin path has no existing ancestor")?
                    .to_os_string();
                suffix.push(name);
                if !current.pop() {
                    bail!("plugin path has no existing ancestor");
                }
            }
            Err(error) => return Err(error).context("resolve plugin file path"),
        }
    }
}
