use super::*;
use anyhow::{bail, Context, Result};
use std::path::{Component, Path};
use std::time::Duration;

const GIT_CLONE_TIMEOUT: Duration = Duration::from_secs(600);
const GIT_REMOTE_URL_MAX_BYTES: usize = 4096;

/// 将真实远端仓库克隆到指定父目录。
///
/// 参数:
/// - `parent`: 已通过服务端允许范围校验的父目录
/// - `remote_url`: Git 支持的 HTTPS、SSH 或本地仓库地址
/// - `directory`: 可选目标目录名，空值时从远端地址推导
///
/// 返回:
/// - Git 输出与克隆后仓库状态
pub(crate) async fn git_clone(
    parent: &Path,
    remote_url: &str,
    directory: Option<&str>,
) -> Result<GitOperationResponse> {
    // 1. 校验父目录、远端地址和单层目标目录名
    let parent = parent
        .canonicalize()
        .with_context(|| format!("clone parent does not exist: {}", parent.display()))?;
    if !parent.is_dir() {
        bail!("clone parent is not a directory: {}", parent.display());
    }
    let remote_url = validate_clone_remote_url(remote_url)?;
    let directory = match directory.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => validate_clone_directory(value)?.to_string(),
        None => derive_clone_directory(remote_url)?,
    };
    let target = parent.join(&directory);
    if target.exists() {
        bail!("clone target already exists: {}", target.display());
    }

    // 2. 使用参数数组执行系统 Git，避免远端地址进入 shell 解析
    let result = run_git_output_with_timeout(
        &parent,
        &["clone", "--progress", "--", remote_url, directory.as_str()],
        GIT_CLONE_TIMEOUT,
    )
    .await;
    operation_response(&target, result, "repository cloned").await
}

/// 校验克隆远端地址长度与控制字符。
///
/// 参数:
/// - `remote_url`: 待校验远端地址
///
/// 返回:
/// - 清理后的远端地址
fn validate_clone_remote_url(remote_url: &str) -> Result<&str> {
    let remote_url = remote_url.trim();
    if remote_url.is_empty() {
        bail!("remote URL cannot be empty");
    }
    if remote_url.len() > GIT_REMOTE_URL_MAX_BYTES {
        bail!("remote URL exceeds the maximum supported length");
    }
    if remote_url.chars().any(char::is_control) {
        bail!("remote URL contains control characters");
    }
    Ok(remote_url)
}

/// 从远端地址推导默认目标目录名。
///
/// 参数:
/// - `remote_url`: 已校验远端地址
///
/// 返回:
/// - 不含 `.git` 后缀的单层目录名
fn derive_clone_directory(remote_url: &str) -> Result<String> {
    let path = remote_url
        .split(['?', '#'])
        .next()
        .unwrap_or(remote_url)
        .trim_end_matches(['/', '\\']);
    let last = path.rsplit(['/', '\\', ':']).next().unwrap_or_default();
    let candidate = last.strip_suffix(".git").unwrap_or(last);
    Ok(validate_clone_directory(candidate)?.to_string())
}

/// 校验克隆目录名仅包含一个普通路径分量。
///
/// 参数:
/// - `directory`: 待校验目录名
///
/// 返回:
/// - 清理后的目录名
fn validate_clone_directory(directory: &str) -> Result<&str> {
    let directory = directory.trim();
    if directory.is_empty() || directory == "." || directory == ".." {
        bail!("clone directory name is invalid");
    }
    if directory.len() > 255 || directory.contains('\0') {
        bail!("clone directory name is invalid");
    }
    let mut components = Path::new(directory).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("clone directory must be a single folder name");
    }
    if directory.contains('/') || directory.contains('\\') {
        bail!("clone directory must be a single folder name");
    }
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_directory_from_https_and_ssh_urls() {
        assert_eq!(
            derive_clone_directory("https://github.com/owner/repository.git").unwrap(),
            "repository"
        );
        assert_eq!(
            derive_clone_directory("git@github.com:owner/repository.git").unwrap(),
            "repository"
        );
    }

    #[test]
    fn rejects_remote_url_control_characters() {
        assert!(validate_clone_remote_url("https://example.com/repo.git\nnext").is_err());
    }
}
