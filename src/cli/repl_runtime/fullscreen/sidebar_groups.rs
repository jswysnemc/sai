//! 全屏右侧栏 Super-Agent 群组概览快照，按轻量轮询周期读取 SQLite 摘要，不阻塞终端事件循环。

use crate::paths::SaiPaths;
use crate::super_agent::model::GroupSummary;
use crate::super_agent::store::SuperAgentStore;
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(in crate::cli::repl_runtime) struct SidebarGroups {
    paths: Option<SaiPaths>,
    pub(in crate::cli::repl_runtime) entries: Vec<GroupSummary>,
    next_refresh: Option<Instant>,
    /// 用户点击侧栏群组条目后待宿主流转打开的目标（`Some(None)` 打开默认群组，`Some(Some(id))` 打开指定群组）
    pub(in crate::cli::repl_runtime) requested: Option<Option<String>>,
}

impl SidebarGroups {
    /// 绑定当前 Sai 路径以便刷新群组列表。
    ///
    /// 参数:
    /// - `paths`: Sai 运行路径
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli::repl_runtime) fn bind(&mut self, paths: &SaiPaths) {
        if self
            .paths
            .as_ref()
            .is_some_and(|existing| existing.state_dir == paths.state_dir)
        {
            return;
        }
        self.paths = Some(paths.clone());
        self.next_refresh = None;
    }

    /// 判断是否已绑定运行路径（未绑定时不渲染该侧栏分组，避免影响隔离单元测试）。
    pub(in crate::cli::repl_runtime) fn is_bound(&self) -> bool {
        self.paths.is_some()
    }

    /// 登记用户点击侧栏群组条目后的打开请求。
    ///
    /// 参数:
    /// - `index`: `None` 表示点击管理入口打开工作台，`Some(i)` 表示打开第 `i` 个群组
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli::repl_runtime) fn request_open(&mut self, index: Option<usize>) {
        let target = index.and_then(|idx| self.entries.get(idx).map(|item| item.group.id.clone()));
        self.requested = Some(target);
    }

    /// 取出待执行的群组工作台打开请求。
    pub(in crate::cli::repl_runtime) fn take_requested(&mut self) -> Option<Option<String>> {
        self.requested.take()
    }

    /// 在侧栏可见时按固定周期刷新群组概览。
    ///
    /// 参数:
    /// - `visible`: 侧栏当前是否可见
    ///
    /// 返回:
    /// - 列表内容是否发生变化
    pub(in crate::cli::repl_runtime) fn tick(&mut self, visible: bool) -> bool {
        let Some(paths) = &self.paths else {
            return false;
        };
        if !visible {
            return false;
        }
        if self.next_refresh.is_some_and(|next| Instant::now() < next) {
            return false;
        }
        self.next_refresh = Some(Instant::now() + REFRESH_INTERVAL);
        let db_path = paths.state_dir.join("super-agent").join("super_agent.db");
        if !db_path.exists() {
            let changed = !self.entries.is_empty();
            self.entries.clear();
            return changed;
        }
        let Ok(store) = SuperAgentStore::open(paths) else {
            return false;
        };
        let Ok(overview) = store.overview() else {
            return false;
        };
        let changed = self.entries.len() != overview.groups.len()
            || self
                .entries
                .iter()
                .zip(&overview.groups)
                .any(|(left, right)| {
                    left.group.id != right.group.id
                        || left.group.name != right.group.name
                        || left.running_worker_count != right.running_worker_count
                        || left.worker_count != right.worker_count
                        || left.open_task_count != right.open_task_count
                });
        self.entries = overview.groups;
        changed
    }

    /// 返回下次刷新等待时长。
    pub(in crate::cli::repl_runtime) fn wait(&self, visible: bool) -> Option<Duration> {
        if self.paths.is_none() || !visible {
            return None;
        }
        Some(self.next_refresh.map_or(Duration::ZERO, |next| {
            next.saturating_duration_since(Instant::now())
        }))
    }
}

impl crate::cli::repl_runtime::ReplRuntime {
    /// 【终端侧栏】【群组绑定】绑定 Sai 路径以刷新侧栏 Super-Agent 群组列表。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn bind_group_browser(&mut self, paths: &SaiPaths) {
        self.group_browser.bind(paths);
    }

    /// 【终端侧栏】【打开群组请求】取出侧栏点击触发的群组工作台打开请求。
    pub(in crate::cli) fn take_open_group_request(&mut self) -> Option<Option<String>> {
        self.group_browser.take_requested()
    }

    /// 【终端侧栏】【群组刷新】按侧栏可见性刷新群组摘要。
    pub(super) fn tick_group_browser(&mut self) -> bool {
        let visible = self.sidebar_visible(usize::from(
            crate::cli::repl_runtime::viewport::TerminalSize::current().cols,
        ));
        self.group_browser.tick(visible)
    }
}
