use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 根据比较模式筛选 Diff 响应中的文件列表。
///
/// 参数:
/// - `state`: 当前仓库状态
/// - `mode`: working_tree、unstaged、staged 或 branch
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - 符合比较模式的文件路径
pub(super) fn diff_files(
    state: &GitRepositoryState,
    mode: &str,
    clean_path: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(path) = clean_path {
        return Ok(vec![path.to_string()]);
    }
    let files = match mode {
        "working_tree" | "branch" => state
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect(),
        "unstaged" => state
            .entries
            .iter()
            .filter(|entry| entry.untracked || entry.conflicted || entry.worktree_status != ".")
            .map(|entry| entry.path.clone())
            .collect(),
        "staged" => state
            .entries
            .iter()
            .filter(|entry| entry.staged && !entry.conflicted && !entry.untracked)
            .map(|entry| entry.path.clone())
            .collect(),
        _ => bail!("unsupported git diff mode: {mode}"),
    };
    Ok(files)
}

/// 读取 HEAD 到工作树的兼容 Diff，并补充未跟踪文本文件。
///
/// 参数:
/// - `state`: 当前仓库状态
/// - `files`: 响应文件列表
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - HEAD 或 ROOT 到 WORKTREE 的 Diff
pub(super) async fn working_tree_diff(
    state: &GitRepositoryState,
    files: Vec<String>,
    clean_path: Option<&str>,
) -> Result<GitDiffResponse> {
    let repo = Path::new(&state.repo_root);
    if !ref_exists(repo, "HEAD").await {
        let patch = append_untracked_patches(repo, state, String::new(), clean_path).await;
        return Ok(diff_response(
            "ROOT",
            "WORKTREE",
            "working_tree",
            files,
            String::new(),
            patch,
        ));
    }
    let output = run_diff(repo, &["HEAD"], clean_path).await?;
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let patch = append_untracked_patches(repo, state, patch, clean_path).await;
    Ok(diff_response(
        "HEAD",
        "WORKTREE",
        "working_tree",
        files,
        stat,
        patch,
    ))
}

/// 读取暂存区到工作树的未暂存 Diff。
///
/// 参数:
/// - `state`: 当前仓库状态
/// - `files`: 响应文件列表
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - INDEX 到 WORKTREE 的 Diff
pub(super) async fn unstaged_diff(
    state: &GitRepositoryState,
    files: Vec<String>,
    clean_path: Option<&str>,
) -> Result<GitDiffResponse> {
    let repo = Path::new(&state.repo_root);
    let output = run_diff(repo, &[], clean_path).await?;
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let patch = append_untracked_patches(repo, state, patch, clean_path).await;
    Ok(diff_response(
        "INDEX", "WORKTREE", "unstaged", files, stat, patch,
    ))
}

/// 读取 HEAD 到暂存区的已暂存 Diff。
///
/// 参数:
/// - `state`: 当前仓库状态
/// - `files`: 响应文件列表
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - HEAD 或 ROOT 到 INDEX 的 Diff
pub(super) async fn staged_diff(
    state: &GitRepositoryState,
    files: Vec<String>,
    clean_path: Option<&str>,
) -> Result<GitDiffResponse> {
    let repo = Path::new(&state.repo_root);
    let output = run_diff(repo, &["--cached"], clean_path).await?;
    let (stat, patch) = split_stat_and_patch(&output.stdout);
    let base = if ref_exists(repo, "HEAD").await {
        "HEAD"
    } else {
        "ROOT"
    };
    Ok(diff_response(base, "INDEX", "staged", files, stat, patch))
}

/// 执行带 stat 与 patch 的 git diff。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `extra_args`: 比较引用或 cached 参数
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - Git 命令输出
async fn run_diff(repo: &Path, extra_args: &[&str], clean_path: Option<&str>) -> Result<GitOutput> {
    let mut args = vec!["diff", "--patch", "--stat"];
    args.extend_from_slice(extra_args);
    if let Some(path) = clean_path {
        args.extend(["--", path]);
    }
    git_success(repo, &args).await
}

/// 将未跟踪文本文件转换为 unified patch 并追加到已有内容。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
/// - `patch`: 已有 Diff
/// - `clean_path`: 可选单文件路径
///
/// 返回:
/// - 补充未跟踪文件后的 Diff
async fn append_untracked_patches(
    repo: &Path,
    state: &GitRepositoryState,
    mut patch: String,
    clean_path: Option<&str>,
) -> String {
    for entry in &state.entries {
        if !entry.untracked || clean_path.is_some_and(|path| entry.path != path) {
            continue;
        }
        if let Ok(Some(part)) = build_untracked_file_patch(repo, &entry.path).await {
            if !patch.is_empty() {
                patch.push('\n');
            }
            patch.push_str(&part);
        }
    }
    patch
}

/// 创建标准 Diff 响应并执行大小截断。
///
/// 参数:
/// - `base_ref`: 比较基线
/// - `head_ref`: 比较目标
/// - `mode`: Diff 模式
/// - `files`: 文件列表
/// - `stat`: 统计文本
/// - `patch`: Diff 文本
///
/// 返回:
/// - 标准 Diff 响应
fn diff_response(
    base_ref: &str,
    head_ref: &str,
    mode: &str,
    files: Vec<String>,
    stat: String,
    patch: String,
) -> GitDiffResponse {
    let (patch, truncated) = truncate_patch(patch);
    GitDiffResponse {
        base_ref: base_ref.to_string(),
        head_ref: head_ref.to_string(),
        mode: mode.to_string(),
        files,
        patch,
        stat,
        truncated,
        binary_files: Vec::new(),
    }
}

/// 拆分 git diff 的 stat 和 unified patch 部分。
///
/// 参数:
/// - `output`: Git 标准输出
///
/// 返回:
/// - stat 与 patch 文本
pub(super) fn split_stat_and_patch(output: &str) -> (String, String) {
    if let Some(index) = output.find("\ndiff --git ") {
        let (stat, patch) = output.split_at(index + 1);
        (stat.trim().to_string(), patch.to_string())
    } else if output.starts_with("diff --git ") {
        (String::new(), output.to_string())
    } else {
        (output.trim().to_string(), String::new())
    }
}

/// 将 Diff 限制在 API 最大响应大小内。
///
/// 参数:
/// - `value`: 原始 Diff
///
/// 返回:
/// - 截断后的文本和是否发生截断
pub(super) fn truncate_patch(value: String) -> (String, bool) {
    if value.len() <= GIT_DIFF_MAX_BYTES {
        return (value, false);
    }
    let mut end = GIT_DIFF_MAX_BYTES.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}\n\n… diff truncated …\n", &value[..end]), true)
}

/// 为未跟踪文本文件构造新增文件 patch。
///
/// 参数:
/// - `repo_root`: 仓库根目录
/// - `path`: 仓库相对路径
///
/// 返回:
/// - 小型文本文件的 patch，目录、二进制或大文件返回空
async fn build_untracked_file_patch(repo_root: &Path, path: &str) -> Result<Option<String>> {
    let clean = validate_repo_relative_path(path)?;
    let absolute = repo_root.join(&clean);
    let metadata = match tokio::fs::metadata(&absolute).await {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if !metadata.is_file() || metadata.len() > 128 * 1024 {
        return Ok(None);
    }
    let bytes = tokio::fs::read(&absolute).await?;
    if bytes.contains(&0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut patch = format!(
        "diff --git a/{clean} b/{clean}\nnew file mode 100644\n--- /dev/null\n+++ b/{clean}\n"
    );
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    patch.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len().max(1)));
    if lines.is_empty() {
        patch.push_str("+\n");
    } else {
        for line in lines {
            patch.push('+');
            patch.push_str(line);
            if !line.ends_with('\n') {
                patch.push('\n');
            }
        }
    }
    Ok(Some(patch))
}
