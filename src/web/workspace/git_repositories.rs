use super::*;
use anyhow::{bail, Context, Result};
use futures_util::stream::{self, StreamExt};
use ignore::WalkBuilder;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const REPOSITORY_SCAN_MAX_DEPTH: usize = 6;
const REPOSITORY_STATUS_CONCURRENCY: usize = 4;

/// 发现工作区内仓库并并发读取轻量摘要。
///
/// 参数:
/// - `workspace_root`: 当前工作区目录
///
/// 返回:
/// - 工作区仓库与 worktree 列表
pub(crate) async fn git_repositories(workspace_root: &Path) -> Result<GitRepositoriesResponse> {
    let workspace_root = canonical_directory(workspace_root)?;
    let roots = discover_repository_roots(&workspace_root).await?;
    let mut repositories = stream::iter(
        roots
            .into_iter()
            .map(|root| async move { repository_summary(root).await }),
    )
    .buffer_unordered(REPOSITORY_STATUS_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;
    repositories.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(GitRepositoriesResponse {
        workspace_root: workspace_root.display().to_string(),
        repositories,
    })
}

/// 校验请求仓库属于当前工作区、其父仓库或关联 worktree。
///
/// 参数:
/// - `workspace_root`: 当前工作区目录
/// - `requested`: 请求中的仓库根目录
///
/// 返回:
/// - 校验后的仓库规范路径
pub(crate) async fn validate_git_repository_root(
    workspace_root: &Path,
    requested: &str,
) -> Result<PathBuf> {
    // 1. 工作区内部仓库和父仓库走快速路径，避免重复目录扫描
    let workspace_root = canonical_directory(workspace_root)?;
    let requested = canonical_directory(Path::new(requested))?;
    if requested.starts_with(&workspace_root) || workspace_root.starts_with(&requested) {
        if let Some(root) = discover_repo(&requested).await? {
            if canonical_directory(&root)? == requested {
                return Ok(requested);
            }
        }
        bail!("requested path is not a Git repository root");
    }
    // 2. 工作区外路径仅接受 Git 已登记的关联 worktree
    let roots = allowed_repository_roots(&workspace_root).await?;
    if roots.iter().any(|root| root == &requested) {
        return Ok(requested);
    }
    bail!("repository is not available in the active workspace")
}

/// 构造单个仓库摘要，状态读取失败时保留可展示错误。
///
/// 参数:
/// - `root`: 仓库根目录
///
/// 返回:
/// - 仓库轻量摘要
async fn repository_summary(root: PathBuf) -> GitRepositorySummary {
    let (state, worktrees) = tokio::join!(git_status(&root), git_worktrees(&root));
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("repository")
        .to_string();
    match state {
        Ok(state) => GitRepositorySummary {
            root: root.display().to_string(),
            name,
            head: state.head,
            ahead: state.ahead,
            behind: state.behind,
            changed: state.entries.len(),
            status: state.status,
            error: state.error,
            worktrees: worktrees.unwrap_or_default(),
        },
        Err(error) => GitRepositorySummary {
            root: root.display().to_string(),
            name,
            head: String::new(),
            ahead: 0,
            behind: 0,
            changed: 0,
            status: "error".to_string(),
            error: Some(error.to_string()),
            worktrees: worktrees.unwrap_or_default(),
        },
    }
}

/// 返回请求可选的仓库根和关联 worktree 根。
///
/// 参数:
/// - `workspace_root`: 当前工作区目录
///
/// 返回:
/// - 允许访问的仓库路径集合
async fn allowed_repository_roots(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let roots = discover_repository_roots(workspace_root).await?;
    let mut allowed = roots.clone();
    for root in roots {
        if let Ok(worktrees) = git_worktrees(&root).await {
            for worktree in worktrees {
                if let Ok(path) = canonical_directory(Path::new(&worktree.path)) {
                    allowed.push(path);
                }
            }
        }
    }
    allowed.sort();
    allowed.dedup();
    Ok(allowed)
}

/// 发现工作区父仓库与有限深度内的嵌套仓库。
///
/// 参数:
/// - `workspace_root`: 规范化工作区目录
///
/// 返回:
/// - 去重后的仓库根目录
async fn discover_repository_roots(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    // 1. 工作区可能位于更上层仓库内部，先登记父仓库
    let mut roots = HashSet::new();
    if let Some(parent_repo) = discover_repo(workspace_root).await? {
        roots.insert(canonical_directory(&parent_repo)?);
    }

    // 2. 目录遍历移入阻塞线程池，避免大型工作区占用 Tokio 执行线程
    let workspace_root = workspace_root.to_path_buf();
    let nested_roots =
        tokio::task::spawn_blocking(move || scan_nested_repository_roots(&workspace_root))
            .await
            .context("repository scan task failed")??;
    roots.extend(nested_roots);
    let mut roots = roots.into_iter().collect::<Vec<_>>();
    roots.sort();
    deduplicate_repository_roots(roots).await
}

/// 在有限深度内同步扫描嵌套仓库。
///
/// 参数:
/// - `workspace_root`: 规范化工作区目录
///
/// 返回:
/// - 扫描得到的仓库根目录
fn scan_nested_repository_roots(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = HashSet::new();
    let walker = WalkBuilder::new(workspace_root)
        .max_depth(Some(REPOSITORY_SCAN_MAX_DEPTH))
        .hidden(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .filter_entry(|entry| should_visit(entry.path(), entry.depth()))
        .build();
    for entry in walker.filter_map(std::result::Result::ok) {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if path.join(".git").exists() {
            roots.insert(canonical_directory(path)?);
        }
    }
    let mut roots = roots.into_iter().collect::<Vec<_>>();
    roots.sort();
    Ok(roots)
}

/// 按 Git 公共目录合并主工作树与 linked worktree 扫描结果。
///
/// 参数:
/// - `roots`: 扫描得到的候选仓库根目录
///
/// 返回:
/// - 每个 Git 公共目录仅保留一个代表仓库
async fn deduplicate_repository_roots(roots: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut grouped = HashMap::<PathBuf, PathBuf>::new();
    for root in roots {
        let common_dir = git_common_directory(&root).await?;
        match grouped.get_mut(&common_dir) {
            Some(existing) if root.join(".git").is_dir() && !existing.join(".git").is_dir() => {
                *existing = root;
            }
            Some(_) => {}
            None => {
                grouped.insert(common_dir, root);
            }
        }
    }
    let mut roots = grouped.into_values().collect::<Vec<_>>();
    roots.sort();
    Ok(roots)
}

/// 读取并规范化仓库的 Git 公共目录。
///
/// 参数:
/// - `root`: 仓库工作树根目录
///
/// 返回:
/// - 主工作树与 linked worktree 共享的 Git 目录
async fn git_common_directory(root: &Path) -> Result<PathBuf> {
    let output = git_success(root, &["rev-parse", "--git-common-dir"]).await?;
    let path = PathBuf::from(output.stdout.trim());
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    canonical_directory(&path)
}

/// 判断仓库扫描是否进入目录。
///
/// 参数:
/// - `path`: 候选目录
/// - `depth`: 相对工作区深度
///
/// 返回:
/// - 应继续扫描时返回 true
fn should_visit(path: &Path, depth: usize) -> bool {
    if depth == 0 {
        return true;
    }
    !matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".git" | "node_modules" | "target" | "dist" | "build" | "coverage" | ".cache")
    )
}

/// 规范化并校验已存在目录。
///
/// 参数:
/// - `path`: 待校验目录
///
/// 返回:
/// - 平台兼容的规范路径
fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = crate::platform::windows_path::canonicalize(path)
        .with_context(|| format!("directory does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("path is not a directory: {}", canonical.display());
    }
    Ok(crate::platform::windows_path::simplified(&canonical))
}
