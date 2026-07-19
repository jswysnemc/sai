use super::*;
use anyhow::{bail, Result};
use futures_util::stream::{self, StreamExt};
use std::path::Path;

const GIT_STATUS_REPOSITORY_LIMIT: usize = 32;
const GIT_STATUS_CONCURRENCY: usize = 4;

/// 并发读取多个仓库或 worktree 的完整状态。
///
/// 参数:
/// - `workspace_root`: 活动工作区目录
/// - `requested_roots`: 前端当前显示的仓库根目录
///
/// 返回:
/// - 按请求顺序排列的仓库状态
pub(crate) async fn git_repository_statuses(
    workspace_root: &Path,
    requested_roots: &[String],
) -> Result<GitRepositoryStatusesResponse> {
    if requested_roots.len() > GIT_STATUS_REPOSITORY_LIMIT {
        bail!("too many Git repositories requested");
    }
    let roots = validate_git_repository_roots(workspace_root, requested_roots).await?;
    let mut indexed = stream::iter(
        roots
            .into_iter()
            .enumerate()
            .map(
                |(index, root)| async move { git_status(&root).await.map(|state| (index, state)) },
            ),
    )
    .buffer_unordered(GIT_STATUS_CONCURRENCY)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    indexed.sort_by_key(|(index, _)| *index);
    Ok(GitRepositoryStatusesResponse {
        repositories: indexed.into_iter().map(|(_, state)| state).collect(),
    })
}
