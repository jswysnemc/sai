use super::*;
use anyhow::Result;
use std::path::Path;

/// 读取指定 stash 的统计和补丁内容。
///
/// 参数:
/// - `root`: 当前工作区或仓库目录
/// - `reference`: stash 引用
///
/// 返回:
/// - 标准 Git Diff 响应
pub(crate) async fn git_stash_diff(root: &Path, reference: &str) -> Result<GitDiffResponse> {
    let reference = validate_stash_ref(Some(reference))?;
    let state = ensure_ready(root).await?;
    let repo = Path::new(&state.repo_root);

    // 1. 读取真实 stash 内容，并包含 stash 中保存的未跟踪文件
    let output = git_success(
        repo,
        &[
            "stash",
            "show",
            "--include-untracked",
            "--stat",
            "--patch",
            "--no-ext-diff",
            &reference,
        ],
    )
    .await?;

    // 2. 沿用 Source Control Diff 的统计拆分和响应大小限制
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let binary = patch.contains("Binary files ");
    let (patch, truncated) = truncate_patch(patch);
    Ok(GitDiffResponse {
        base_ref: format!("{reference}^"),
        head_ref: reference.clone(),
        mode: "stash".to_string(),
        files: Vec::new(),
        patch,
        stat,
        truncated,
        binary_files: if binary { vec![reference] } else { Vec::new() },
    })
}
