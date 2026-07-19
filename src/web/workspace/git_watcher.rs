use super::*;
use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

const GIT_WATCH_DEBOUNCE: Duration = Duration::from_millis(300);
const GIT_WATCH_EVENT_PATH_LIMIT: usize = 64;

/// Git 仓库文件变化事件。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct GitWatchEvent {
    pub(crate) sequence: u64,
    pub(crate) workspace_root: String,
    pub(crate) paths: Vec<String>,
    pub(crate) paths_truncated: bool,
    pub(crate) repository_metadata_changed: bool,
    pub(crate) error: Option<String>,
}

enum WatcherMessage {
    Paths(Vec<PathBuf>),
    Error(String),
}

/// 监听工作区、选中仓库和真实 Git 元数据目录。
pub(crate) struct RepositoryWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::UnboundedReceiver<WatcherMessage>,
    workspace_root: String,
    sequence: u64,
}

impl RepositoryWatcher {
    /// 启动跨平台 Git 仓库监听器。
    ///
    /// 参数:
    /// - `workspace_root`: 活动工作区根目录
    /// - `repository_root`: 可选当前仓库或 worktree 根目录
    ///
    /// 返回:
    /// - 已注册监听目录的仓库监听器
    pub(crate) async fn start(
        workspace_root: &Path,
        repository_root: Option<&Path>,
    ) -> Result<Self> {
        // 1. 工作区始终递归监听，以便发现新仓库和普通文件变化
        let workspace_root = canonical_watch_directory(workspace_root)?;
        let mut watch_roots = vec![workspace_root.clone()];
        // 2. 外部 worktree 和真实 Git 目录需要单独监听
        if let Some(repository_root) = repository_root {
            let repository_root = canonical_watch_directory(repository_root)?;
            watch_roots.push(repository_root.clone());
            watch_roots.extend(git_metadata_directories(&repository_root).await?);
        }
        let watch_roots = normalize_watch_roots(watch_roots);
        let (sender, receiver) = mpsc::unbounded_channel();
        let watcher = tokio::task::spawn_blocking(move || create_watcher(watch_roots, sender))
            .await
            .context("Git repository watcher task failed")??;
        Ok(Self {
            _watcher: watcher,
            receiver,
            workspace_root: workspace_root.display().to_string(),
            sequence: 0,
        })
    }

    /// 等待并合并下一批仓库文件变化。
    ///
    /// 返回:
    /// - 监听器关闭时返回 `None`，否则返回 300ms 内合并后的事件
    pub(crate) async fn next_event(&mut self) -> Option<GitWatchEvent> {
        let first = self.receiver.recv().await?;
        let mut paths = BTreeSet::new();
        let mut errors = Vec::new();
        merge_watcher_message(first, &mut paths, &mut errors);

        // 1. 固定窗口合并高频编辑和 Git 锁文件变化
        let delay = sleep(GIT_WATCH_DEBOUNCE);
        tokio::pin!(delay);
        loop {
            tokio::select! {
                message = self.receiver.recv() => {
                    let Some(message) = message else { break };
                    merge_watcher_message(message, &mut paths, &mut errors);
                }
                _ = &mut delay => break,
            }
        }

        // 2. 限制事件载荷，查询刷新只依赖变化类型而不依赖完整路径列表
        self.sequence = self.sequence.saturating_add(1);
        let paths_truncated = paths.len() > GIT_WATCH_EVENT_PATH_LIMIT;
        let repository_metadata_changed = paths.iter().any(|path| is_git_metadata_path(path));
        let paths = paths
            .into_iter()
            .take(GIT_WATCH_EVENT_PATH_LIMIT)
            .map(|path| path.display().to_string())
            .collect();
        Some(GitWatchEvent {
            sequence: self.sequence,
            workspace_root: self.workspace_root.clone(),
            paths,
            paths_truncated,
            repository_metadata_changed,
            error: (!errors.is_empty()).then(|| errors.join("\n")),
        })
    }
}

/// 创建并注册底层文件系统监听器。
///
/// 参数:
/// - `watch_roots`: 去重后的监听目录
/// - `sender`: 文件事件发送端
///
/// 返回:
/// - 保持监听生命周期的系统监听器
fn create_watcher(
    watch_roots: Vec<PathBuf>,
    sender: mpsc::UnboundedSender<WatcherMessage>,
) -> Result<RecommendedWatcher> {
    let callback_sender = sender.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let message = match result {
            Ok(event) if is_refresh_event(&event.kind) => WatcherMessage::Paths(event.paths),
            Ok(_) => return,
            Err(error) => WatcherMessage::Error(error.to_string()),
        };
        let _ = callback_sender.send(message);
    })?;
    for root in watch_roots {
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch Git repository path: {}", root.display()))?;
    }
    Ok(watcher)
}

/// 读取仓库实际使用的 Git 目录和公共 Git 目录。
///
/// 参数:
/// - `repository_root`: 仓库或 worktree 根目录
///
/// 返回:
/// - 规范化且存在的 Git 元数据目录
async fn git_metadata_directories(repository_root: &Path) -> Result<Vec<PathBuf>> {
    let output = git_success(
        repository_root,
        &["rev-parse", "--git-dir", "--git-common-dir"],
    )
    .await?;
    output
        .stdout
        .lines()
        .map(|value| {
            let path = PathBuf::from(value.trim());
            let path = if path.is_absolute() {
                path
            } else {
                repository_root.join(path)
            };
            canonical_watch_directory(&path)
        })
        .collect()
}

/// 删除已经由祖先目录递归覆盖的监听路径。
///
/// 参数:
/// - `roots`: 候选监听目录
///
/// 返回:
/// - 由少到多排列且不存在包含关系的目录
fn normalize_watch_roots(mut roots: Vec<PathBuf>) -> Vec<PathBuf> {
    roots.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    let mut normalized = Vec::<PathBuf>::new();
    for root in roots {
        if normalized.iter().any(|existing| root.starts_with(existing)) {
            continue;
        }
        normalized.push(root);
    }
    normalized
}

/// 合并单条底层监听消息。
///
/// 参数:
/// - `message`: 路径变化或监听错误
/// - `paths`: 合并后的路径集合
/// - `errors`: 合并后的错误集合
///
/// 返回:
/// - 无
fn merge_watcher_message(
    message: WatcherMessage,
    paths: &mut BTreeSet<PathBuf>,
    errors: &mut Vec<String>,
) {
    match message {
        WatcherMessage::Paths(changed) => paths.extend(changed),
        WatcherMessage::Error(error) => errors.push(error),
    }
}

/// 判断底层事件是否需要刷新 Git 状态。
///
/// 参数:
/// - `kind`: 文件系统事件类型
///
/// 返回:
/// - 访问事件返回 false，其余变化返回 true
fn is_refresh_event(kind: &EventKind) -> bool {
    !matches!(kind, EventKind::Access(_))
}

/// 判断变化路径是否属于 Git 元数据。
///
/// 参数:
/// - `path`: 变化文件或目录路径
///
/// 返回:
/// - 路径任一层名称为 `.git` 时返回 true
fn is_git_metadata_path(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == OsStr::new(".git"))
}

/// 规范化已存在的监听目录。
///
/// 参数:
/// - `path`: 待监听目录
///
/// 返回:
/// - 平台兼容的规范目录路径
fn canonical_watch_directory(path: &Path) -> Result<PathBuf> {
    let canonical = crate::platform::windows_path::canonicalize(path)
        .with_context(|| format!("watch directory does not exist: {}", path.display()))?;
    if !canonical.is_dir() {
        anyhow::bail!("watch path is not a directory: {}", canonical.display());
    }
    Ok(crate::platform::windows_path::simplified(&canonical))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    /// 验证祖先目录递归监听时删除重复子目录。
    #[test]
    fn removes_nested_watch_roots() {
        let roots = normalize_watch_roots(vec![
            PathBuf::from("workspace/repo/.git"),
            PathBuf::from("workspace"),
            PathBuf::from("external"),
        ]);

        assert_eq!(
            roots,
            vec![PathBuf::from("external"), PathBuf::from("workspace")]
        );
    }

    /// 验证普通访问事件不会触发 Git 查询刷新。
    #[test]
    fn ignores_access_events() {
        assert!(!is_refresh_event(&EventKind::Access(
            notify::event::AccessKind::Any
        )));
        assert!(is_refresh_event(&EventKind::Modify(
            notify::event::ModifyKind::Any
        )));
    }

    /// 验证真实文件修改会在防抖后生成事件。
    #[tokio::test]
    async fn reports_workspace_file_changes() {
        let temp = tempfile::tempdir().unwrap();
        let mut watcher = RepositoryWatcher::start(temp.path(), None).await.unwrap();
        let target = temp.path().join("tracked.txt");
        tokio::fs::write(&target, "changed\n").await.unwrap();

        let event = timeout(Duration::from_secs(5), watcher.next_event())
            .await
            .unwrap()
            .unwrap();

        assert!(event.paths.iter().any(|path| path.ends_with("tracked.txt")));
        assert!(event.error.is_none());
    }

    /// 验证 Git CLI 修改索引时标记元数据变化。
    #[tokio::test]
    async fn reports_git_metadata_changes() {
        let temp = tempfile::tempdir().unwrap();
        git_success(temp.path(), &["init", "-b", "main"])
            .await
            .unwrap();
        let mut watcher = RepositoryWatcher::start(temp.path(), Some(temp.path()))
            .await
            .unwrap();
        tokio::fs::write(temp.path().join("tracked.txt"), "changed\n")
            .await
            .unwrap();
        git_success(temp.path(), &["add", "--", "tracked.txt"])
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(5), watcher.next_event())
            .await
            .unwrap()
            .unwrap();

        assert!(event.repository_metadata_changed);
        assert!(event.error.is_none());
    }
}
