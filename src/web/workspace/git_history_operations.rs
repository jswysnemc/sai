use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 以 detached HEAD 方式检出指定提交。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 提交引用或哈希
///
/// 返回:
/// - Git 命令输出
pub(super) async fn checkout_commit(repo: &Path, commit: Option<&str>) -> Result<GitOutput> {
    let commit = validate_commit(repo, commit).await?;
    git_success(repo, &["switch", "--detach", &commit]).await
}

/// 将指定提交拣选到当前分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 提交引用或哈希
///
/// 返回:
/// - Git 命令输出
pub(super) async fn cherry_pick_commit(repo: &Path, commit: Option<&str>) -> Result<GitOutput> {
    let commit = validate_commit(repo, commit).await?;
    git_success(repo, &["cherry-pick", &commit]).await
}

/// 将当前分支变基到指定提交。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 目标提交引用或哈希
///
/// 返回:
/// - Git 命令输出
pub(super) async fn rebase_onto_commit(repo: &Path, commit: Option<&str>) -> Result<GitOutput> {
    let commit = validate_commit(repo, commit).await?;
    git_success(repo, &["rebase", &commit]).await
}

/// 使用 soft、mixed 或 hard 模式重置当前分支。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 目标提交引用或哈希
/// - `mode`: 重置模式
///
/// 返回:
/// - Git 命令输出
pub(super) async fn reset_commit(
    repo: &Path,
    commit: Option<&str>,
    mode: Option<&str>,
) -> Result<GitOutput> {
    let commit = validate_commit(repo, commit).await?;
    let flag = match mode.unwrap_or("mixed") {
        "soft" => "--soft",
        "mixed" => "--mixed",
        "hard" => "--hard",
        value => bail!("unsupported reset mode: {value}"),
    };
    git_success(repo, &["reset", flag, &commit]).await
}

/// 创建一个还原指定提交的新提交。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 待还原提交引用或哈希
///
/// 返回:
/// - Git 命令输出
pub(super) async fn revert_commit(repo: &Path, commit: Option<&str>) -> Result<GitOutput> {
    let commit = validate_commit(repo, commit).await?;
    git_success(repo, &["revert", "--no-edit", &commit]).await
}

/// 校验提交引用存在并解析为完整哈希。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `commit`: 提交引用或哈希
///
/// 返回:
/// - 完整提交哈希
async fn validate_commit(repo: &Path, commit: Option<&str>) -> Result<String> {
    let commit = commit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("commit cannot be empty"))?;
    let revision = format!("{commit}^{{commit}}");
    let output = git_success(repo, &["rev-parse", "--verify", &revision]).await?;
    let sha = output.stdout.trim();
    if sha.is_empty() {
        bail!("unable to resolve commit: {commit}");
    }
    Ok(sha.to_string())
}
