use super::*;
use anyhow::{bail, Result};
use std::path::Path;

/// 新增或更新 origin 远端地址。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `remote_url`: 远端地址
///
/// 返回:
/// - Git 命令输出
pub(super) async fn set_origin_remote(repo: &Path, remote_url: Option<&str>) -> Result<GitOutput> {
    let remote_url = validate_remote_url(remote_url)?;
    if git_origin_exists(repo).await {
        git_success(repo, &["remote", "set-url", "origin", remote_url]).await
    } else {
        git_success(repo, &["remote", "add", "origin", remote_url]).await
    }
}

/// 保存工作树修改到 stash。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `message`: 可选 stash 说明
/// - `include_untracked`: 是否包含未跟踪文件
///
/// 返回:
/// - Git 命令输出
pub(super) async fn stash_push(
    repo: &Path,
    message: Option<&str>,
    include_untracked: bool,
) -> Result<GitOutput> {
    let mut args = vec!["stash", "push"];
    if include_untracked {
        args.push("--include-untracked");
    }
    let owned;
    if let Some(value) = message.map(str::trim).filter(|value| !value.is_empty()) {
        owned = value.to_string();
        args.extend(["-m", owned.as_str()]);
    }
    git_success(repo, &args).await
}

/// 应用指定 stash，但保留 stash 记录。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `reference`: stash 引用
///
/// 返回:
/// - Git 命令输出
pub(super) async fn stash_apply(repo: &Path, reference: Option<&str>) -> Result<GitOutput> {
    let reference = validate_stash_ref(reference)?;
    git_success(repo, &["stash", "apply", &reference]).await
}

/// 弹出指定 stash。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `reference`: 可选 stash 引用，空值使用最新记录
///
/// 返回:
/// - Git 命令输出
pub(super) async fn stash_pop(repo: &Path, reference: Option<&str>) -> Result<GitOutput> {
    if let Some(reference) = reference {
        let reference = validate_stash_ref(Some(reference))?;
        git_success(repo, &["stash", "pop", &reference]).await
    } else {
        git_success(repo, &["stash", "pop"]).await
    }
}

/// 删除指定 stash 记录。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `reference`: stash 引用
///
/// 返回:
/// - Git 命令输出
pub(super) async fn stash_drop(repo: &Path, reference: Option<&str>) -> Result<GitOutput> {
    let reference = validate_stash_ref(reference)?;
    git_success(repo, &["stash", "drop", &reference]).await
}

/// 创建轻量标签。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `tag`: 标签名称
/// - `commit`: 可选目标提交，空值使用 HEAD
///
/// 返回:
/// - Git 命令输出
pub(super) async fn create_tag(
    repo: &Path,
    tag: Option<&str>,
    commit: Option<&str>,
) -> Result<GitOutput> {
    let tag = validate_tag(repo, tag).await?;
    let commit = validate_commit(repo, Some(commit.unwrap_or("HEAD"))).await?;
    git_success(repo, &["tag", &tag, &commit]).await
}

/// 删除本地标签。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `tag`: 标签名称
///
/// 返回:
/// - Git 命令输出
pub(super) async fn delete_tag(repo: &Path, tag: Option<&str>) -> Result<GitOutput> {
    let tag = validate_tag(repo, tag).await?;
    git_success(repo, &["tag", "-d", &tag]).await
}

/// 新增命名远端。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `name`: 远端名称
/// - `remote_url`: 远端地址
///
/// 返回:
/// - Git 命令输出
pub(super) async fn add_remote(
    repo: &Path,
    name: Option<&str>,
    remote_url: Option<&str>,
) -> Result<GitOutput> {
    let name = validate_remote_name(repo, name).await?;
    let remote_url = validate_remote_url(remote_url)?;
    git_success(repo, &["remote", "add", &name, remote_url]).await
}

/// 删除命名远端及其跟踪引用。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `name`: 远端名称
///
/// 返回:
/// - Git 命令输出
pub(super) async fn remove_remote(repo: &Path, name: Option<&str>) -> Result<GitOutput> {
    let name = validate_remote_name(repo, name).await?;
    git_success(repo, &["remote", "remove", &name]).await
}

/// 校验 stash 引用仅采用 stash@{数字} 格式。
///
/// 参数:
/// - `reference`: stash 引用
///
/// 返回:
/// - 清理后的 stash 引用
pub(super) fn validate_stash_ref(reference: Option<&str>) -> Result<String> {
    let reference = reference
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("stash reference cannot be empty"))?;
    let index = reference
        .strip_prefix("stash@{")
        .and_then(|value| value.strip_suffix('}'))
        .filter(|value| {
            !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
        });
    if index.is_none() {
        bail!("invalid stash reference");
    }
    Ok(reference.to_string())
}

/// 校验标签名称符合 Git 引用规则。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `tag`: 标签名称
///
/// 返回:
/// - 清理后的标签名称
async fn validate_tag(repo: &Path, tag: Option<&str>) -> Result<String> {
    let tag = tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("tag name cannot be empty"))?;
    git_success(repo, &["check-ref-format", &format!("refs/tags/{tag}")]).await?;
    Ok(tag.to_string())
}

/// 校验远端名称符合 Git 引用规则。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `name`: 远端名称
///
/// 返回:
/// - 清理后的远端名称
async fn validate_remote_name(repo: &Path, name: Option<&str>) -> Result<String> {
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("remote name cannot be empty"))?;
    git_success(
        repo,
        &["check-ref-format", &format!("refs/remotes/{name}/probe")],
    )
    .await?;
    Ok(name.to_string())
}

/// 校验远端地址非空。
///
/// 参数:
/// - `remote_url`: 远端地址
///
/// 返回:
/// - 清理后的远端地址
fn validate_remote_url(remote_url: Option<&str>) -> Result<&str> {
    remote_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("remote URL cannot be empty"))
}
