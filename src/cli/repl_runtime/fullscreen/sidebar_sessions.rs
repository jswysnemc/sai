//! 当前会话与近期会话目录，磁盘读取不占用终端事件循环。

use crate::paths::SaiPaths;
use crate::state::LocatedSession;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub(in crate::cli::repl_runtime) const REFRESH: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::cli::repl_runtime) struct SessionEntry {
    pub(in crate::cli::repl_runtime) id: String,
    pub(in crate::cli::repl_runtime) title: String,
    pub(in crate::cli::repl_runtime) state_dir: PathBuf,
    pub(in crate::cli::repl_runtime) updated_at: String,
    pub(in crate::cli::repl_runtime) status: SessionStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli::repl_runtime) enum SessionStatus {
    Running,
    Idle,
    Recent,
}

#[derive(Default)]
pub(in crate::cli::repl_runtime) struct SidebarSessions {
    pub(in crate::cli::repl_runtime) paths: Option<SaiPaths>,
    pub(in crate::cli::repl_runtime) workspace: PathBuf,
    pub(in crate::cli::repl_runtime) current: Option<SessionEntry>,
    pub(in crate::cli::repl_runtime) entries: Vec<SessionEntry>,
    catalog: Option<Receiver<Result<Vec<SessionEntry>, String>>>,
    next_refresh: Option<Instant>,
}

impl SidebarSessions {
    /// 【终端侧栏】【会话绑定】参数为路径与实际执行会话；返回无，不依赖共享当前指针推断身份
    pub(in crate::cli::repl_runtime) fn bind(
        &mut self,
        paths: &SaiPaths,
        id: &str,
        state_dir: PathBuf,
    ) {
        if self
            .current
            .as_ref()
            .is_some_and(|entry| entry.state_dir == state_dir)
        {
            return;
        }
        self.paths = Some(paths.clone());
        self.workspace = crate::runtime_cwd::current_dir().unwrap_or_default();
        let cached = self
            .entries
            .iter()
            .find(|entry| entry.state_dir == state_dir)
            .cloned();
        self.current = Some(cached.unwrap_or(SessionEntry {
            id: id.into(),
            title: id.into(),
            state_dir,
            updated_at: String::new(),
            status: SessionStatus::Idle,
        }));
        self.catalog = None;
        self.next_refresh = None;
    }

    /// 【终端侧栏】【会话采样】参数 visible 表示面板可见；返回列表是否改变，索引和在线状态在后台查询
    pub(in crate::cli::repl_runtime) fn tick(&mut self, visible: bool) -> bool {
        let mut changed = false;
        if let Some(receiver) = &self.catalog {
            match receiver.try_recv() {
                Ok(Ok(entries)) => {
                    if let Some(current) = &mut self.current {
                        if let Some(entry) = entries
                            .iter()
                            .find(|entry| entry.state_dir == current.state_dir)
                        {
                            *current = entry.clone();
                        }
                    }
                    changed = self.entries != entries;
                    self.entries = entries;
                    self.catalog = None;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.catalog = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if visible
            && self.catalog.is_none()
            && self.next_refresh.is_none_or(|at| Instant::now() >= at)
        {
            if let Some(paths) = self.paths.clone() {
                let workspace = self.workspace.clone();
                let (sender, receiver) = mpsc::channel();
                self.catalog = Some(receiver);
                self.next_refresh = Some(Instant::now() + REFRESH);
                std::thread::spawn(move || {
                    let result =
                        crate::state::list_located_sessions_for_workspace(&paths, &workspace)
                            .map(|entries| entries.into_iter().take(24).map(entry).collect())
                            .map_err(|error| error.to_string());
                    let _ = sender.send(result);
                });
            }
        }
        changed
    }

    /// 【终端侧栏】【刷新等待】无参数；返回下一次唤醒时间，待处理查询使用短轮询
    pub(in crate::cli::repl_runtime) fn wait(&self) -> Option<Duration> {
        self.paths.as_ref()?;
        Some(if self.catalog.is_some() {
            Duration::from_millis(50)
        } else {
            self.next_refresh
                .map_or(REFRESH, |at| at.saturating_duration_since(Instant::now()))
                .max(Duration::from_millis(50))
        })
    }

    /// 【终端侧栏】【浏览标识】无参数；返回独立浏览目标，缺省为正在执行的当前会话
    pub(in crate::cli::repl_runtime) fn viewed_id(&self) -> Option<&str> {
        self.current.as_ref().map(|entry| entry.id.as_str())
    }
}

/// 【终端侧栏】【状态投影】参数 located 为工作区索引条目；返回标题与真实运行、在线或近期状态
fn entry(located: LocatedSession) -> SessionEntry {
    let status = if crate::runner::active_run(&located.state_dir).is_some() {
        SessionStatus::Running
    } else if !crate::runner::session_instances(&located.state_dir).is_empty() {
        SessionStatus::Idle
    } else {
        SessionStatus::Recent
    };
    SessionEntry {
        id: located.info.id,
        title: located.info.title,
        updated_at: located.info.updated_at,
        state_dir: located.state_dir,
        status,
    }
}
