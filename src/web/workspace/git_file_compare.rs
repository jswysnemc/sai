use super::*;
use anyhow::{bail, Context, Result};
use std::path::Path;

/// 比较仓库工作树中的两个普通文件。
///
/// 参数:
/// - `root`: 当前工作区或仓库目录
/// - `base_path`: 基准文件的仓库相对路径
/// - `head_path`: 目标文件的仓库相对路径
///
/// 返回:
/// - 标准 Git Diff 响应
pub(crate) async fn git_file_compare(
    root: &Path,
    base_path: &str,
    head_path: &str,
) -> Result<GitDiffResponse> {
    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);
    let base_path = validate_repo_relative_path(base_path)?;
    let head_path = validate_repo_relative_path(head_path)?;
    if base_path == head_path {
        bail!("file comparison requires two different paths");
    }

    // 1. 校验真实路径，阻止父目录符号链接越出仓库
    validate_comparison_file(repo, &base_path).await?;
    validate_comparison_file(repo, &head_path).await?;

    // 2. `--no-index` 以退出码 1 表示存在差异，该状态不属于命令失败
    let output = git_raw(
        repo,
        &[
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--stat",
            "--patch",
            "--",
            base_path.as_str(),
            head_path.as_str(),
        ],
    )
    .await?;
    if !matches!(output.status.code(), Some(0 | 1)) {
        let stderr = trim_bytes(&output.stderr);
        let stdout = trim_bytes(&output.stdout);
        let message = if stderr.is_empty() { stdout } else { stderr };
        bail!(if message.is_empty() {
            "git file comparison failed".to_string()
        } else {
            message
        });
    }

    // 3. 沿用 Source Control Diff 的统计拆分和响应大小限制
    let (stat, patch) = split_stat_and_patch(&trim_bytes(&output.stdout));
    let binary = patch.contains("Binary files ");
    let (patch, truncated) = truncate_patch(patch);
    Ok(GitDiffResponse {
        base_ref: base_path.clone(),
        head_ref: head_path.clone(),
        mode: "files".to_string(),
        files: vec![base_path.clone(), head_path.clone()],
        patch,
        stat,
        truncated,
        binary_files: if binary {
            vec![base_path, head_path]
        } else {
            Vec::new()
        },
    })
}

/// 校验比较文件存在、属于仓库且不是符号链接。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 已完成词法校验的仓库相对路径
///
/// 返回:
/// - 文件可以安全读取时返回空结果
async fn validate_comparison_file(repo: &Path, path: &str) -> Result<()> {
    let target = repo.join(path);
    let metadata = tokio::fs::symlink_metadata(&target)
        .await
        .with_context(|| format!("failed to inspect comparison file {path}"))?;
    if metadata.file_type().is_symlink() {
        bail!("comparison file cannot be a symbolic link: {path}");
    }
    if !metadata.is_file() {
        bail!("comparison path is not a regular file: {path}");
    }
    let canonical_repo = tokio::fs::canonicalize(repo)
        .await
        .context("failed to resolve repository root")?;
    let canonical_target = tokio::fs::canonicalize(&target)
        .await
        .with_context(|| format!("failed to resolve comparison file {path}"))?;
    if !canonical_target.starts_with(&canonical_repo) {
        bail!("comparison file resolves outside repository: {path}");
    }
    Ok(())
}
