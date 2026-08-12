use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

const EXTRA_ROOTS_ENV: &str = "SAI_WEB_WORKSPACE_ROOTS";

/// 服务端可选择的目录条目。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub git_repository: bool,
}

/// 服务端目录浏览结果；`roots` 为快捷入口目录而非浏览边界。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DirectoryListing {
    pub current: String,
    pub parent: Option<String>,
    pub roots: Vec<DirectoryEntry>,
    pub entries: Vec<DirectoryEntry>,
}

/// 浏览服务端任意目录的一级子目录。
///
/// 参数:
/// - `requested`: 可选绝对目录，空值使用第一个快捷入口目录
///
/// 返回:
/// - 当前目录、父目录、快捷入口和子目录列表
pub(crate) fn browse(requested: Option<&str>) -> Result<DirectoryListing> {
    let roots = quick_access_roots()?;
    let requested = requested.map(str::trim).filter(|value| !value.is_empty());
    let current = match requested {
        Some(value) => canonical_browsable_directory(Path::new(value))?,
        None => roots
            .first()
            .cloned()
            .context("no workspace roots are available")?,
    };
    // 目录不可读（如权限不足）时返回带路径的可读错误，而不是裸 IO 错误
    let mut entries = std::fs::read_dir(&current)
        .with_context(|| format!("failed to read directory: {}", display_path(&current)))?
        .filter_map(Result::ok)
        .filter_map(|entry| directory_entry(entry.path()).ok())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    // merged-usr 系统中 /bin、/sbin 等符号链接规范化到同一目标，按路径去重避免重复条目
    entries.dedup_by(|left, right| left.path == right.path);
    let parent = resolve_parent(&current);
    Ok(DirectoryListing {
        current: display_path(&current),
        parent,
        roots: roots
            .iter()
            .filter_map(|root| directory_entry(root.clone()).ok())
            .collect(),
        entries,
    })
}

/// 在服务端指定父目录下创建子目录。
///
/// 参数:
/// - `parent`: 父目录绝对路径，必须真实存在
/// - `name`: 新目录名，不允许包含路径分隔符或 `..`
///
/// 返回:
/// - 新目录对应的目录条目
pub(crate) fn create_directory(parent: &str, name: &str) -> Result<DirectoryEntry> {
    // 1. 校验目录名合法性
    let name = name.trim();
    if name.is_empty() {
        bail!("directory name is empty");
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\u{5c}') {
        bail!("directory name contains illegal characters");
    }
    // 2. 规范化父目录并确认真实存在
    let parent = canonical_browsable_directory(Path::new(parent.trim()))?;
    // 3. 创建子目录并返回条目
    let target = parent.join(name);
    if target.exists() {
        bail!("directory already exists: {}", target.display());
    }
    std::fs::create_dir(&target)
        .with_context(|| format!("failed to create directory: {}", target.display()))?;
    directory_entry(target)
}

/// 校验目录在服务端真实存在且可作为浏览或操作目标。
///
/// 参数:
/// - `requested`: 待校验绝对目录
///
/// 返回:
/// - 规范化后的目录
pub(crate) fn validate_browsable_directory(requested: &str) -> Result<PathBuf> {
    canonical_browsable_directory(Path::new(requested.trim()))
}

/// 返回目录浏览器的快捷入口集合（不再限制浏览范围）。
///
/// 返回:
/// - 用户主目录、服务端当前目录、`SAI_WEB_WORKSPACE_ROOTS` 追加目录，以及文件系统根
fn quick_access_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(home) =
        directories::BaseDirs::new().map(|directories| directories.home_dir().to_path_buf())
    {
        push_root(&mut roots, home);
    }
    // 当前目录可能已被删除，失败时跳过而不是中断整个浏览
    if let Ok(current) = std::env::current_dir() {
        push_root(&mut roots, current);
    }
    if let Ok(value) = std::env::var(EXTRA_ROOTS_ENV) {
        for item in std::env::split_paths(&value) {
            push_root(&mut roots, item);
        }
    }
    for root in filesystem_roots() {
        push_root(&mut roots, root);
    }
    if roots.is_empty() {
        bail!("no readable workspace roots are configured");
    }
    Ok(roots)
}

/// 返回文件系统根目录集合（Unix 为 `/`，Windows 为各可用盘符根）。
fn filesystem_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        (b'A'..=b'Z')
            .map(|letter| PathBuf::from(format!("{}:{}", letter as char, '\u{5c}')))
            .filter(|path| path.is_dir())
            .collect()
    } else {
        vec![PathBuf::from("/")]
    }
}

/// 添加规范化且不重复的根目录。
fn push_root(roots: &mut Vec<PathBuf>, path: PathBuf) {
    let Ok(path) = path.canonicalize() else {
        return;
    };
    if path.is_dir() && !roots.iter().any(|root| root == &path) {
        roots.push(path);
    }
}

/// 规范化目录路径并做基础健壮性校验。
///
/// 参数:
/// - `path`: 待校验目录路径
///
/// 返回:
/// - 规范化后的目录；不存在或不是目录时返回错误
fn canonical_browsable_directory(path: &Path) -> Result<PathBuf> {
    // 1. 规范化会跟随符号链接并要求目标真实存在，Windows 下可接受盘符与正斜杠写法
    let canonical = path
        .canonicalize()
        .with_context(|| format!("directory does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("path is not a directory: {}", canonical.display());
    }
    Ok(canonical)
}

/// 构造服务端目录条目。
fn directory_entry(path: PathBuf) -> Result<DirectoryEntry> {
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        bail!("path is not a directory");
    }
    let display = display_path(&canonical);
    // 文件系统根（如 `/` 或盘符根）没有文件名，用显示路径本身作为条目名
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| display.clone());
    Ok(DirectoryEntry {
        name,
        git_repository: canonical.join(".git").is_dir(),
        path: display,
    })
}

/// 解析当前目录的上级路径。
///
/// 参数:
/// - `current`: 已规范化的当前目录
///
/// 返回:
/// - 上级目录；已到文件系统根（如 `/` 或盘符根）时返回 None
fn resolve_parent(current: &Path) -> Option<String> {
    current.parent().map(display_path)
}

/// 输出给前端的路径字符串（去掉 Windows 扩展前缀，便于回退与输入）。
fn display_path(path: &Path) -> String {
    strip_verbatim_prefix(&path.display().to_string())
}

/// 去掉 Windows 扩展路径前缀。
fn strip_verbatim_prefix(value: &str) -> String {
    strip_windows_verbatim(value).unwrap_or_else(|| value.to_string())
}

/// 剥离 Windows 扩展路径前缀；非匹配时返回 None。
fn strip_windows_verbatim(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    // \\?\UNC\
    const UNC: &[u8] = &[0x5c, 0x5c, 0x3f, 0x5c, b'U', b'N', b'C', 0x5c];
    // \\?\
    const VERBATIM: &[u8] = &[0x5c, 0x5c, 0x3f, 0x5c];
    if bytes.starts_with(UNC) {
        let rest = &value[UNC.len()..];
        let mut out = String::new();
        out.push('\u{5c}');
        out.push('\u{5c}');
        out.push_str(rest);
        return Some(out);
    }
    if bytes.starts_with(VERBATIM) {
        return Some(value[VERBATIM.len()..].to_string());
    }
    if let Some(rest) = value.strip_prefix("//?/UNC/") {
        let mut out = String::new();
        out.push('\u{5c}');
        out.push('\u{5c}');
        for ch in rest.chars() {
            out.push(if ch == '/' { '\u{5c}' } else { ch });
        }
        return Some(out);
    }
    if let Some(rest) = value.strip_prefix("//?/") {
        return Some(
            rest.chars()
                .map(|ch| if ch == '/' { '\u{5c}' } else { ch })
                .collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_any_existing_directory() {
        // 放宽后任意真实目录都可浏览，不再受根目录白名单限制
        let outside = tempfile::tempdir().unwrap();
        let canonical = canonical_browsable_directory(outside.path()).unwrap();
        assert_eq!(canonical, outside.path().canonicalize().unwrap());
    }

    #[test]
    fn rejects_missing_path_and_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(canonical_browsable_directory(&dir.path().join("missing")).is_err());
        let file = dir.path().join("plain.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(canonical_browsable_directory(&file).is_err());
    }

    #[test]
    fn parent_walks_up_to_filesystem_root() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let current = child.canonicalize().unwrap();
        let parent = resolve_parent(&current).unwrap();
        assert_eq!(
            PathBuf::from(&parent).canonicalize().unwrap(),
            root.path().canonicalize().unwrap()
        );
        if cfg!(unix) {
            assert!(resolve_parent(Path::new("/")).is_none());
        }
    }

    #[test]
    fn quick_access_roots_include_filesystem_root() {
        let roots = quick_access_roots().unwrap();
        assert!(!roots.is_empty());
        if cfg!(unix) {
            assert!(roots.iter().any(|root| root == Path::new("/")));
        }
    }

    #[test]
    fn browse_reaches_filesystem_root() {
        if !cfg!(unix) {
            return;
        }
        let listing = browse(Some("/")).unwrap();
        assert_eq!(listing.current, "/");
        assert!(listing.parent.is_none());
        assert!(listing.roots.iter().any(|root| root.path == "/"));
    }

    #[test]
    fn strip_verbatim_prefix_removes_windows_extended_form() {
        let input = String::from_utf8(vec![
            0x5c, 0x5c, 0x3f, 0x5c, b'C', b':', 0x5c, b'U', b's', b'e', b'r', b's', 0x5c, b'd',
            b'e', b'm', b'o',
        ])
        .unwrap();
        let expected = String::from_utf8(vec![
            b'C', b':', 0x5c, b'U', b's', b'e', b'r', b's', 0x5c, b'd', b'e', b'm', b'o',
        ])
        .unwrap();
        assert_eq!(strip_verbatim_prefix(&input), expected);
        assert_eq!(strip_verbatim_prefix("/home/demo"), "/home/demo");
    }
}
