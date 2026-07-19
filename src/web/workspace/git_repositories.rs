use super::*;
use anyhow::{bail, Context, Result};
use futures_util::stream::{self, StreamExt};
use ignore::WalkBuilder;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const REPOSITORY_SCAN_MAX_DEPTH: usize = 6;
const REPOSITORY_STATUS_CONCURRENCY: usize = 4;

/// 工作区仓库与 worktree 探测选项。
#[derive(Clone, Copy)]
pub(crate) struct GitRepositoryDiscoveryOptions {
    pub auto_repository_detection: bool,
    pub detect_worktrees: bool,
    pub detect_worktrees_limit: usize,
}

impl Default for GitRepositoryDiscoveryOptions {
    fn default() -> Self {
        Self {
            auto_repository_detection: true,
            detect_worktrees: true,
            detect_worktrees_limit: 10,
        }
    }
}

/// 按配置发现工作区仓库并并发读取轻量摘要。
///
/// 参数:
/// - `workspace_root`: 当前工作区目录
/// - `options`: 自动探测与 worktree 限制
///
/// 返回:
/// - 工作区仓库与 worktree 列表
pub(crate) async fn git_repositories_with_options(
    workspace_root: &Path,
    options: GitRepositoryDiscoveryOptions,
) -> Result<GitRepositoriesResponse> {
    let workspace_root = canonical_directory(workspace_root)?;
    let roots = if options.auto_repository_detection {
        discover_repository_roots(&workspace_root).await?
    } else {
        discover_current_repository_root(&workspace_root).await?
    };
    let mut repositories = stream::iter(
        roots
            .into_iter()
            .map(|root| async move { repository_summary(root, options).await }),
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
    let roots = validate_git_repository_roots(workspace_root, &[requested.to_string()]).await?;
    roots.into_iter().next().context("repository root is empty")
}

/// 批量校验请求仓库属于当前工作区、其父仓库或关联 worktree。
///
/// 参数:
/// - `workspace_root`: 当前工作区目录
/// - `requested`: 请求中的仓库根目录列表
///
/// 返回:
/// - 按请求顺序去重后的仓库规范路径
pub(crate) async fn validate_git_repository_roots(
    workspace_root: &Path,
    requested: &[String],
) -> Result<Vec<PathBuf>> {
    // 1. 先规范化全部请求，并判断是否需要扫描外部关联 worktree
    let workspace_root = canonical_directory(workspace_root)?;
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for value in requested {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let path = canonical_directory(Path::new(value))?;
        if seen.insert(path.clone()) {
            candidates.push(path);
        }
    }
    let needs_external_scan = candidates
        .iter()
        .any(|path| !path.starts_with(&workspace_root) && !workspace_root.starts_with(path));
    let allowed = if needs_external_scan {
        Some(allowed_repository_roots(&workspace_root).await?)
    } else {
        None
    };

    // 2. 内部路径使用快速 Git 根校验，外部路径复用一次关联 worktree 扫描结果
    for path in &candidates {
        if path.starts_with(&workspace_root) || workspace_root.starts_with(path) {
            if let Some(root) = discover_repo(path).await? {
                if canonical_directory(&root)? == *path {
                    continue;
                }
            }
            bail!(
                "requested path is not a Git repository root: {}",
                path.display()
            );
        }
        if allowed
            .as_ref()
            .is_some_and(|roots| roots.iter().any(|root| root == path))
        {
            continue;
        }
        bail!(
            "repository is not available in the active workspace: {}",
            path.display()
        );
    }
    Ok(candidates)
}

/// 构造单个仓库摘要，状态读取失败时保留可展示错误。
///
/// 参数:
/// - `root`: 仓库根目录
///
/// 返回:
/// - 仓库轻量摘要
async fn repository_summary(
    root: PathBuf,
    options: GitRepositoryDiscoveryOptions,
) -> GitRepositorySummary {
    let (state, worktrees) = tokio::join!(
        git_status(&root),
        configured_worktrees(
            &root,
            options.detect_worktrees,
            options.detect_worktrees_limit
        )
    );
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

/// 按配置读取并截断关联 worktree。
///
/// 参数:
/// - `root`: 仓库根目录
/// - `enabled`: 是否执行 worktree 命令
/// - `limit`: 最大返回数量
///
/// 返回:
/// - 受限的 worktree 列表
async fn configured_worktrees(
    root: &Path,
    enabled: bool,
    limit: usize,
) -> Result<Vec<GitWorktree>> {
    if !enabled {
        return Ok(Vec::new());
    }
    let mut worktrees = git_worktrees(root).await?;
    worktrees.truncate(limit);
    Ok(worktrees)
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

/// 仅发现包含当前工作区的直接仓库，不扫描嵌套目录。
///
/// 参数:
/// - `workspace_root`: 规范化工作区目录
///
/// 返回:
/// - 零个或一个仓库根目录
async fn discover_current_repository_root(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    match discover_repo(workspace_root).await? {
        Some(root) => Ok(vec![canonical_directory(&root)?]),
        None => Ok(Vec::new()),
    }
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
