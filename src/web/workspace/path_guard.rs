use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

/// 解析并校验已经存在的工作区路径。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 工作区相对路径
///
/// 返回:
/// - 规范化后的绝对路径
pub(super) fn existing_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let relative = normalize_request_path(&root, relative)?;
    let relative = validate_relative(&relative)?;
    let path = root.join(relative);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("path does not exist: {}", path.display()))?;
    if !canonical.starts_with(&root) {
        bail!("path escapes workspace root");
    }
    Ok(canonical)
}

/// 解析允许创建的新文件路径。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 工作区相对路径
///
/// 返回:
/// - 经过父目录校验的绝对路径
pub(super) fn writable_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let relative = normalize_request_path(&root, relative)?;
    let relative = validate_relative(&relative)?;
    let path = root.join(relative);
    let parent = path
        .parent()
        .context("file path has no parent directory")?
        .canonicalize()
        .with_context(|| format!("parent directory does not exist: {}", path.display()))?;
    if !parent.starts_with(&root) {
        bail!("path escapes workspace root");
    }
    Ok(path)
}

/// 解析允许重命名或删除的现有工作区条目。
///
/// 参数:
/// - `root`: 工作区根目录
/// - `relative`: 工作区相对路径
///
/// 返回:
/// - 不解析最终符号链接的绝对路径
pub(super) fn mutable_existing_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let relative = normalize_request_path(&root, relative)?;
    let relative = validate_relative(&relative)?;
    if relative == Path::new(".") {
        bail!("workspace root cannot be modified");
    }
    let path = root.join(relative);
    let parent = path
        .parent()
        .context("entry path has no parent directory")?
        .canonicalize()?;
    if !parent.starts_with(&root) {
        bail!("path escapes workspace root");
    }
    std::fs::symlink_metadata(&path)
        .with_context(|| format!("path does not exist: {}", path.display()))?;
    Ok(path)
}

/// 校验浏览器提交的路径为普通相对路径。
fn validate_relative(value: &str) -> Result<PathBuf> {
    let value = value.trim();
    let path = if value.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(value)
    };
    if path.is_absolute() {
        bail!("absolute paths are not allowed");
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!("parent path components are not allowed");
    }
    Ok(path)
}

/// 把工作区根内的绝对路径归一化为相对路径，其余原样返回。
///
/// 参数:
/// - `root`: 规范化后的工作区根目录
/// - `value`: 浏览器提交的路径
///
/// 返回:
/// - 相对路径字符串；根外的绝对路径返回错误
fn normalize_request_path(root: &Path, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if !Path::new(trimmed).is_absolute() {
        return Ok(trimmed.to_string());
    }
    // 1. 绝对路径仅在位于工作区根内时被接受，并转换为相对路径
    let stripped = Path::new(trimmed)
        .strip_prefix(root)
        .map_err(|_| anyhow::anyhow!("absolute path is outside the workspace root"))?;
    Ok(stripped.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_and_absolute_paths() {
        let absolute = std::env::temp_dir().join("sai-absolute-test");
        assert!(validate_relative("../secret").is_err());
        assert!(validate_relative(absolute.to_str().unwrap()).is_err());
        assert!(validate_relative("src/main.rs").is_ok());
    }

    #[test]
    fn rejects_workspace_root_mutation() {
        let temp = tempfile::tempdir().unwrap();
        assert!(mutable_existing_path(temp.path(), "").is_err());
    }

    #[test]
    fn resolves_absolute_path_inside_root() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        let inside = root.join("main.rs");
        // 1. 工作区内的绝对路径应被接受并解析
        assert!(existing_path(temp.path(), inside.to_str().unwrap()).is_ok());
        // 2. 工作区外的绝对路径应被拒绝
        assert!(existing_path(temp.path(), outside.path().to_str().unwrap()).is_err());
    }
}
