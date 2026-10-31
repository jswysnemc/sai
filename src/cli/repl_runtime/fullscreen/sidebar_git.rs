//! Git 概览通过后台任务获取，绘制线程只读取快照。

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const QUERY_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct GitSummary {
    pub(super) state: String,
    pub(super) branch: String,
    pub(super) files: Vec<(String, String)>,
    pub(super) staged: usize,
    pub(super) unstaged: usize,
    pub(super) untracked: usize,
    pub(super) conflicted: usize,
    pub(super) changes: std::collections::BTreeMap<String, crate::web::GitFileStats>,
}

#[derive(Default)]
pub(super) struct GitWatcher {
    pub(super) summary: GitSummary,
    directory: Option<PathBuf>,
    receiver: Option<Receiver<GitSummary>>,
    task: Option<tokio::task::JoinHandle<()>>,
    next_poll: Option<Instant>,
}

impl GitWatcher {
    /// 【终端侧栏】【Git 刷新】捕获目录并启动异步查询，目录变化时丢弃旧任务结果。
    /// 参数: directory 为当前工作目录；返回: 快照是否变化。
    pub(super) fn tick(&mut self, directory: PathBuf) -> bool {
        let now = Instant::now();
        let mut changed = false;
        if self.directory.as_ref() != Some(&directory) {
            if let Some(task) = self.task.take() {
                task.abort();
            }
            self.directory = Some(directory.clone());
            self.receiver = None;
            self.next_poll = None;
            self.summary = GitSummary::default();
            changed = true;
        }
        if let Some(summary) = self
            .receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            changed |= self.summary != summary;
            self.summary = summary;
            self.receiver = None;
            self.task = None;
            self.next_poll = Some(now + POLL_INTERVAL);
        }
        if self.receiver.is_none() && self.next_poll.is_none_or(|deadline| now >= deadline) {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let (sender, receiver) = mpsc::channel();
                self.receiver = Some(receiver);
                self.task = Some(handle.spawn(async move {
                    let summary = read_summary(directory).await;
                    let _ = sender.send(summary);
                }));
            }
        }
        changed
    }

    /// 【终端侧栏】【Git 等待】后台查询期间短周期采样，空闲时按刷新周期唤醒。
    /// 参数: 无；返回: 下一次检查前的等待时间。
    pub(super) fn wait(&self) -> Duration {
        if self.receiver.is_some() {
            Duration::from_millis(100)
        } else {
            self.next_poll.map_or(Duration::ZERO, |deadline| {
                deadline.saturating_duration_since(Instant::now())
            })
        }
    }
}

impl Drop for GitWatcher {
    /// 【终端侧栏】【Git 收尾】离开全屏后取消未完成的只读查询。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// 【终端侧栏】【Git 快照】复用既有工作树状态接口，不获取 diff 正文或执行写入操作。
/// 参数: directory 为查询目录；返回: 紧凑文件状态，失败时提供明确状态。
pub(super) async fn read_summary(directory: PathBuf) -> GitSummary {
    let result = tokio::time::timeout(QUERY_TIMEOUT, async {
        let state = crate::web::git_status(&directory).await?;
        let changes = if state.status == "ready" {
            crate::web::git_file_stats(&directory)
                .await
                .unwrap_or_default()
        } else {
            Default::default()
        };
        Ok::<_, anyhow::Error>((state, changes))
    })
    .await;
    match result {
        Ok(Ok((state, changes))) => GitSummary {
            state: state.status,
            branch: state.head,
            files: state
                .entries
                .into_iter()
                .map(|entry| {
                    let status = if entry.conflicted {
                        "UU".into()
                    } else {
                        format!("{}{}", entry.index_status, entry.worktree_status).replace('.', " ")
                    };
                    (status, entry.path)
                })
                .collect(),
            staged: state.dirty_counts.staged,
            unstaged: state.dirty_counts.unstaged,
            untracked: state.dirty_counts.untracked,
            conflicted: state.dirty_counts.conflicted,
            changes,
        },
        _ => GitSummary {
            state: "error".into(),
            ..Default::default()
        },
    }
}

#[cfg(test)]
#[path = "sidebar_git_tests.rs"]
mod tests;
