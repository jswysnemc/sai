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

/// 服务端目录浏览结果。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DirectoryListing {
    pub current: String,
    pub parent: Option<String>,
    pub roots: Vec<DirectoryEntry>,
    pub entries: Vec<DirectoryEntry>,
}

/// 浏览服务端允许根目录中的一级子目录。
///
/// 参数:
/// - `requested`: 可选绝对目录，空值使用第一个允许根目录
///
/// 返回:
/// - 当前目录、父目录、根目录和子目录列表
pub(crate) fn browse(requested: Option<&str>) -> Result<DirectoryListing> {
    let roots = allowed_roots()?;
    let requested = requested.map(str::trim).filter(|value| !value.is_empty());
    let current = match requested {
        Some(value) => canonical_allowed_directory(Path::new(value), &roots)?,
        None => roots
            .first()
            .cloned()
            .context("no workspace roots are available")?,
    };
    let mut entries = std::fs::read_dir(&current)?
        .filter_map(Result::ok)
        .filter_map(|entry| directory_entry(entry.path()).ok())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    let parent = resolve_allowed_parent(&current, &roots);
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

/// 在允许根目录内的父目录下创建子目录。
///
/// 参数:
/// - `parent`: 父目录绝对路径，必须位于允许根目录内
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
    // 2. 校验父目录位于允许根目录内
    let roots = allowed_roots()?;
    let parent = canonical_allowed_directory(Path::new(parent.trim()), &roots)?;
    // 3. 创建子目录并返回条目
    let target = parent.join(name);
    if target.exists() {
        bail!("directory already exists: {}", target.display());
    }
    std::fs::create_dir(&target)
        .with_context(|| format!("failed to create directory: {}", target.display()))?;
    directory_entry(target)
}

/// 校验目录位于服务端允许浏览的根目录内。
///
/// 参数:
/// - `requested`: 待校验绝对目录
///
/// 返回:
/// - 规范化后的允许目录
pub(crate) fn validate_browsable_directory(requested: &str) -> Result<PathBuf> {
    let roots = allowed_roots()?;
    canonical_allowed_directory(Path::new(requested.trim()), &roots)
}

/// 返回配置后的服务端目录根集合。
fn allowed_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(home) =
        directories::BaseDirs::new().map(|directories| directories.home_dir().to_path_buf())
    {
        push_root(&mut roots, home);
    }
    push_root(&mut roots, std::env::current_dir()?);
    if let Ok(value) = std::env::var(EXTRA_ROOTS_ENV) {
        for item in std::env::split_paths(&value) {
            push_root(&mut roots, item);
        }
    }
    if roots.is_empty() {
        bail!("no readable workspace roots are configured");
    }
    Ok(roots)
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

/// 校验目录处于允许根目录中。
fn canonical_allowed_directory(path: &Path, roots: &[PathBuf]) -> Result<PathBuf> {
    // 1. 先规范化，Windows 下可接受盘符与正斜杠写法
    let canonical = path
        .canonicalize()
        .with_context(|| format!("directory does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("path is not a directory: {}", canonical.display());
    }
    // 2. 允许根内判断：同时比较原始规范化路径与去掉扩展前缀后的路径
    if !roots.iter().any(|root| path_is_within(&canonical, root)) {
        bail!("directory is outside configured workspace roots");
    }
    Ok(canonical)
}

/// 判断 path 是否位于 root 之下（含 root 自身）。
fn path_is_within(path: &Path, root: &Path) -> bool {
    if path.starts_with(root) || paths_equal(path, root) {
        return true;
    }
    let path_text = strip_verbatim_prefix(&path.display().to_string());
    let root_text = strip_verbatim_prefix(&root.display().to_string());
    if cfg!(windows) {
        let path_norm = normalize_windows_text(&path_text);
        let root_norm = normalize_windows_text(&root_text);
        if path_norm == root_norm {
            return true;
        }
        let mut prefix = root_norm;
        prefix.push('\u{5c}');
        path_norm.starts_with(&prefix)
    } else {
        let path_norm = path_text.trim_end_matches('/');
        let root_norm = root_text.trim_end_matches('/');
        path_norm == root_norm || path_norm.starts_with(&(root_norm.to_string() + "/"))
    }
}

/// Windows 路径文本归一化：统一分隔符并去掉末尾分隔符。
fn normalize_windows_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch == '/' { '\u{5c}' } else { ch })
        .collect::<String>()
        .trim_end_matches('\u{5c}')
        .to_ascii_lowercase()
}

/// 构造服务端目录条目。
fn directory_entry(path: PathBuf) -> Result<DirectoryEntry> {
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        bail!("path is not a directory");
    }
    let name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("/")
        .to_string();
    Ok(DirectoryEntry {
        name,
        git_repository: canonical.join(".git").is_dir(),
        path: display_path(&canonical),
    })
}

/// 解析当前目录在允许根内的上级路径。
///
/// 参数:
/// - `current`: 已规范化的当前目录
/// - `roots`: 允许根目录
///
/// 返回:
/// - 可浏览的上级目录；已在根边界时返回 None
fn resolve_allowed_parent(current: &Path, roots: &[PathBuf]) -> Option<String> {
    // 1. 若当前就是某个允许根，禁止再向上跳出
    if roots.iter().any(|root| paths_equal(root, current)) {
        return None;
    }
    // 2. 逐级向上找到仍落在任一允许根内的父目录
    let mut cursor = current.parent().map(Path::to_path_buf);
    while let Some(parent) = cursor {
        if let Ok(canonical) = canonical_allowed_directory(&parent, roots) {
            return Some(display_path(&canonical));
        }
        if roots.iter().any(|root| paths_equal(root, &parent)) {
            return Some(display_path(&parent));
        }
        cursor = parent.parent().map(Path::to_path_buf);
    }
    None
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
        return Some(rest.chars().map(|ch| if ch == '/' { '\u{5c}' } else { ch }).collect());
    }
    None
}

/// 比较两个路径是否指向同一位置（忽略 Windows 扩展前缀差异）。
fn paths_equal(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    strip_verbatim_prefix(&left.display().to_string())
        .eq_ignore_ascii_case(&strip_verbatim_prefix(&right.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_directory_outside_allowed_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let roots = vec![root.path().canonicalize().unwrap()];
        assert!(canonical_allowed_directory(outside.path(), &roots).is_err());
    }

    #[test]
    fn parent_stops_at_allowed_root() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let roots = vec![root.path().canonicalize().unwrap()];
        let current = child.canonicalize().unwrap();
        let parent = resolve_allowed_parent(&current, &roots).unwrap();
        assert_eq!(PathBuf::from(&parent).canonicalize().unwrap(), roots[0]);
        assert!(resolve_allowed_parent(&roots[0], &roots).is_none());
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
