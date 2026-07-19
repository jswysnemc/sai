use super::*;
use anyhow::{bail, Context, Result};
use std::path::Component;
use std::path::{Path, PathBuf};

/// 读取仓库关联的全部 worktree。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - Git worktree 列表
pub(super) async fn git_worktrees(repo: &Path) -> Result<Vec<GitWorktree>> {
    let output = git_success(repo, &["worktree", "list", "--porcelain", "-z"]).await?;
    let current = simplified_existing_path(repo)?;
    Ok(parse_worktrees(&output.stdout, &current))
}

/// 创建新的 Git worktree。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `workspace_root`: 活动工作区根目录
/// - `path`: 新 worktree 路径
/// - `branch`: 可选现有分支或新分支起点
/// - `new_branch`: 可选新分支名称
///
/// 返回:
/// - Git 命令输出
pub(super) async fn add_worktree(
    repo: &Path,
    workspace_root: &Path,
    path: Option<&str>,
    branch: Option<&str>,
    new_branch: Option<&str>,
) -> Result<GitOutput> {
    // 1. 创建目标必须尚不存在，避免 Git 覆盖已有目录
    let target = new_worktree_target(repo, workspace_root, path)?;
    if target.exists() {
        bail!("worktree path already exists: {}", target.display());
    }
    // 2. 按新分支或现有分支模式组装真实 git worktree add 参数
    let target = target.display().to_string();
    let branch = normalized_optional(branch);
    let new_branch = normalized_optional(new_branch);
    let mut args = vec!["worktree".to_string(), "add".to_string()];
    if let Some(new_branch) = new_branch {
        args.push("-b".to_string());
        args.push(new_branch.to_string());
    }
    args.push(target);
    if let Some(branch) = branch {
        args.push(branch.to_string());
    }
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_success(repo, &refs).await
}

/// 解析并校验新 worktree 的创建路径。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `workspace_root`: 活动工作区根目录
/// - `path`: 用户输入路径
///
/// 返回:
/// - 位于工作区或其同级目录范围内的绝对路径
fn new_worktree_target(repo: &Path, workspace_root: &Path, path: Option<&str>) -> Result<PathBuf> {
    let raw = path.map(str::trim).filter(|value| !value.is_empty());
    let raw = raw.context("worktree path cannot be empty")?;
    let input = Path::new(raw);
    let absolute = input.is_absolute();
    if input.components().any(|component| match component {
        Component::ParentDir => true,
        Component::RootDir | Component::Prefix(_) => !absolute,
        _ => false,
    }) {
        bail!("parent path components are not allowed in worktree paths");
    }

    // 1. 相对路径保持 Git worktree 的同级目录习惯
    let target = if input.is_absolute() {
        input.to_path_buf()
    } else {
        repo.parent().unwrap_or(repo).join(input)
    };
    // 2. 规范化目标父目录，阻止符号链接和绝对路径逃逸允许范围
    let workspace_root = simplified_existing_path(workspace_root)?;
    let allowed_root = workspace_root.parent().unwrap_or(&workspace_root);
    let parent = target
        .parent()
        .context("worktree path has no parent directory")?;
    let parent = simplified_existing_path(parent)?;
    if !parent.starts_with(allowed_root) {
        bail!(
            "worktree path is outside the active workspace scope: {}",
            target.display()
        );
    }
    Ok(parent.join(
        target
            .file_name()
            .context("worktree path has no directory name")?,
    ))
}

/// 移除 Git 已登记的 worktree。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 待移除 worktree 路径
/// - `force`: 是否强制移除含未提交改动的 worktree
///
/// 返回:
/// - Git 命令输出
pub(super) async fn remove_worktree(
    repo: &Path,
    path: Option<&str>,
    force: bool,
) -> Result<GitOutput> {
    // 1. 只接受 Git worktree 列表中已登记的目标
    let requested = worktree_target(repo, path)?;
    let worktrees = git_worktrees(repo).await?;
    let matched = worktrees
        .iter()
        .find(|worktree| same_path(Path::new(&worktree.path), &requested))
        .context("worktree is not registered in this repository")?;
    if matched.current {
        bail!("cannot remove the current worktree");
    }
    // 2. 默认保留 Git 的脏工作树保护，仅显式 force 时强制移除
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(matched.path.as_str());
    git_success(repo, &args).await
}

/// 解析 `git worktree list --porcelain` 输出。
///
/// 参数:
/// - `raw`: Git porcelain 文本
/// - `current`: 当前仓库规范路径
///
/// 返回:
/// - worktree 摘要列表
fn parse_worktrees(raw: &str, current: &Path) -> Vec<GitWorktree> {
    raw.split("\0\0")
        .filter_map(|block| parse_worktree_block(block, current))
        .collect()
}

/// 解析单个 worktree porcelain 段落。
///
/// 参数:
/// - `block`: 单个 porcelain 段落
/// - `current`: 当前仓库规范路径
///
/// 返回:
/// - 可识别时返回 worktree 摘要
fn parse_worktree_block(block: &str, current: &Path) -> Option<GitWorktree> {
    let mut path = String::new();
    let mut head = String::new();
    let mut branch = String::new();
    let mut bare = false;
    let mut detached = false;
    let mut locked = false;
    let mut prunable = false;
    for line in block.split('\0') {
        if let Some(value) = line.strip_prefix("worktree ") {
            path = value.to_string();
        } else if let Some(value) = line.strip_prefix("HEAD ") {
            head = value.to_string();
        } else if let Some(value) = line.strip_prefix("branch ") {
            branch = value
                .strip_prefix("refs/heads/")
                .unwrap_or(value)
                .to_string();
        } else if line == "bare" {
            bare = true;
        } else if line == "detached" {
            detached = true;
        } else if line.starts_with("locked") {
            locked = true;
        } else if line.starts_with("prunable") {
            prunable = true;
        }
    }
    if path.is_empty() {
        return None;
    }
    Some(GitWorktree {
        current: same_path(Path::new(&path), current),
        path,
        head,
        branch,
        bare,
        detached,
        locked,
        prunable,
    })
}

/// 将 worktree 输入转换为绝对路径。
///
/// 参数:
/// - `repo`: 仓库根目录
/// - `path`: 用户输入路径
///
/// 返回:
/// - 绝对 worktree 路径
fn worktree_target(repo: &Path, path: Option<&str>) -> Result<PathBuf> {
    let raw = path.map(str::trim).filter(|value| !value.is_empty());
    let raw = raw.context("worktree path cannot be empty")?;
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(repo.parent().unwrap_or(repo).join(path))
}

/// 规范化可选非空文本。
///
/// 参数:
/// - `value`: 可选文本
///
/// 返回:
/// - 去除首尾空白后的非空文本
fn normalized_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// 规范化已存在的路径。
///
/// 参数:
/// - `path`: 待规范化目录
///
/// 返回:
/// - 平台兼容的规范路径
fn simplified_existing_path(path: &Path) -> Result<PathBuf> {
    let canonical = crate::platform::windows_path::canonicalize(path)
        .with_context(|| format!("path does not exist: {}", path.display()))?;
    Ok(crate::platform::windows_path::simplified(&canonical))
}

/// 判断两个路径是否表示同一位置。
///
/// 参数:
/// - `left`: 左侧路径
/// - `right`: 右侧路径
///
/// 返回:
/// - 路径相同时返回 true
fn same_path(left: &Path, right: &Path) -> bool {
    let left = simplified_existing_path(left).unwrap_or_else(|_| left.to_path_buf());
    let right = simplified_existing_path(right).unwrap_or_else(|_| right.to_path_buf());
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 porcelain 解析保留分支与状态标记。
    #[test]
    fn parses_worktree_porcelain() {
        let raw = "worktree /repo\0HEAD abc123\0branch refs/heads/main\0\0worktree /repo-feature\0HEAD def456\0detached\0locked reason\0\0";
        let worktrees = parse_worktrees(raw, Path::new("/repo"));

        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].branch, "main");
        assert!(worktrees[0].current);
        assert!(worktrees[1].detached);
        assert!(worktrees[1].locked);
    }
}
