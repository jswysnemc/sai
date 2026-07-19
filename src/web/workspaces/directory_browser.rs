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
    let parent = current.parent().and_then(|parent| {
        canonical_allowed_directory(parent, &roots)
            .ok()
            .map(|path| path.display().to_string())
    });
    Ok(DirectoryListing {
        current: current.display().to_string(),
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
    if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
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
    let canonical = path
        .canonicalize()
        .with_context(|| format!("directory does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("path is not a directory: {}", canonical.display());
    }
    if !roots.iter().any(|root| canonical.starts_with(root)) {
        bail!("directory is outside configured workspace roots");
    }
    Ok(canonical)
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
        path: canonical.display().to_string(),
    })
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
}
