use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 切换本地或远端分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `branch`: 分支完整名称
/// - `kind`: 可选分支类型
///
/// 返回:
/// - Git 命令输出
pub(super) async fn switch_branch(
    repo: &Path,
    branch: Option<&str>,
    kind: Option<&str>,
) -> Result<GitOutput> {
    let branch = validate_branch_name(repo, branch).await?;
    let is_remote = match kind {
        Some("remote") => true,
        Some("local") => false,
        _ => branch.contains('/') && remote_ref_exists(repo, &branch).await,
    };
    if !is_remote {
        return git_success(repo, &["switch", &branch]).await;
    }
    let local = remote_local_name(&branch)?;
    if branch_exists_local(repo, local).await {
        git_success(repo, &["switch", local]).await
    } else {
        git_success(repo, &["switch", "-c", local, "--track", &branch]).await
    }
}

/// 创建并切换到本地分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `branch`: 新分支名称
/// - `start_point`: 可选起点引用
///
/// 返回:
/// - Git 命令输出
pub(super) async fn create_branch(
    repo: &Path,
    branch: Option<&str>,
    start_point: Option<&str>,
) -> Result<GitOutput> {
    let branch = validate_branch_name(repo, branch).await?;
    if let Some(start_point) = start_point.filter(|value| !value.trim().is_empty()) {
        git_success(
            repo,
            &[
                "rev-parse",
                "--verify",
                &format!("{start_point}^{{commit}}"),
            ],
        )
        .await?;
        git_success(repo, &["switch", "-c", &branch, start_point]).await
    } else {
        git_success(repo, &["switch", "-c", &branch]).await
    }
}

/// 重命名本地分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `branch`: 原分支名称
/// - `new_branch`: 新分支名称
///
/// 返回:
/// - Git 命令输出
pub(super) async fn rename_branch(
    repo: &Path,
    branch: Option<&str>,
    new_branch: Option<&str>,
) -> Result<GitOutput> {
    let branch = validate_branch_name(repo, branch).await?;
    let new_branch = validate_branch_name(repo, new_branch).await?;
    git_success(repo, &["branch", "-m", &branch, &new_branch]).await
}

/// 删除非当前本地分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `state`: 当前仓库状态
/// - `branch`: 待删除分支
/// - `force`: 是否强制删除
///
/// 返回:
/// - Git 命令输出
pub(super) async fn delete_branch(
    repo: &Path,
    state: &GitRepositoryState,
    branch: Option<&str>,
    force: bool,
) -> Result<GitOutput> {
    let branch = validate_branch_name(repo, branch).await?;
    if branch == state.head {
        bail!("cannot delete the currently checked out branch");
    }
    let flag = if force { "-D" } else { "-d" };
    git_success(repo, &["branch", flag, &branch]).await
}

/// 校验分支名称。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `branch`: 待校验分支名称
///
/// 返回:
/// - 清理后的分支名称
async fn validate_branch_name(repo: &Path, branch: Option<&str>) -> Result<String> {
    let branch = branch
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("branch name cannot be empty"))?;
    git_success(repo, &["check-ref-format", "--branch", branch]).await?;
    Ok(branch.to_string())
}

/// 从远端引用计算本地分支名称。
///
/// 参数:
/// - `branch`: 远端分支完整名称
///
/// 返回:
/// - 本地分支名称
fn remote_local_name(branch: &str) -> Result<&str> {
    branch
        .split_once('/')
        .map(|(_, local)| local)
        .filter(|local| !local.is_empty())
        .ok_or_else(|| anyhow::anyhow!("remote branch must include the remote name"))
}

#[cfg(test)]
mod tests {
    use super::remote_local_name;

    #[test]
    fn keeps_nested_remote_branch_path() {
        assert_eq!(
            remote_local_name("upstream/feature/editor").unwrap(),
            "feature/editor"
        );
    }
}
