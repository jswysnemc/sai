use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 为当前分支新增 origin 并发布上游分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 发布前仓库状态
/// - `remote_url`: 目标远端地址
///
/// 返回:
/// - 新增远端和首次推送的合并输出
pub(super) async fn publish_repository(
    repo: &Path,
    state: &GitRepositoryState,
    remote_url: Option<&str>,
) -> Result<GitOutput> {
    if !state.has_commits || !ref_exists(repo, "HEAD").await {
        bail!("create a commit before publishing the repository");
    }
    if git_origin_exists(repo).await {
        bail!("origin remote already exists");
    }

    // 1. 先保存 origin，使失败后的状态仍可由用户检查和修正
    let remote = set_origin_remote(repo, remote_url).await?;
    // 2. 首次推送当前分支并建立 upstream
    let refreshed = git_status(repo).await?;
    let pushed = push_repo(repo, &refreshed).await?;
    Ok(merge_outputs([remote, pushed]))
}
