//! 后台任务真实接口适配和可取消的异步读取。

use super::model::{Output, Task};
use crate::tools::command::{
    cleanup_managed_background_tasks, list_managed_background_tasks,
    read_managed_background_output, stop_background_task_for_user,
};
use crate::{config::AppConfig, i18n::text as t, paths::SaiPaths};
use anyhow::Result;
use futures_util::FutureExt;
use serde::Deserialize;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Request {
    Refresh,
    Stop { id: String, force: bool },
    Cleanup,
}
pub(super) struct Update {
    pub tasks: Vec<Task>,
    pub output: Option<(String, std::result::Result<Output, String>)>,
    pub notice: Option<String>,
}
pub(super) struct TaskService {
    paths: SaiPaths,
    config: AppConfig,
    pending: Option<JoinHandle<Result<Update>>>,
    mutation: bool,
}

impl TaskService {
    /// 【后台任务】【服务创建】保存路径与配置快照，不启动额外运行时。
    /// 参数: paths 为路径，config 为配置；返回: 服务实例。
    pub fn new(paths: &SaiPaths, config: &AppConfig) -> Self {
        Self {
            paths: paths.clone(),
            config: config.clone(),
            pending: None,
            mutation: false,
        }
    }
    /// 【后台任务】【请求状态】检查是否有未处理请求。
    /// 参数: 无；返回: 请求是否存在。
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    /// 【后台任务】【写入状态】检查是否正在停止或清理任务。
    /// 参数: 无；返回: 写操作是否进行中。
    pub fn mutating(&self) -> bool {
        self.busy() && self.mutation
    }
    /// 【后台任务】【请求启动】写操作前取消旧读取，不并发执行多个写操作。
    /// 参数: request 为操作，selected 为当前选择；返回: 无。
    pub fn start(&mut self, request: Request, selected: Option<String>) {
        self.start_with_output(request, selected, true);
    }
    /// 【后台任务】【概览读取】侧栏只读取任务列表，打开管理器后才读取日志。
    /// 参数: request 为操作，selected 为选择，output 决定是否读取日志；返回: 无。
    pub fn start_with_output(&mut self, request: Request, selected: Option<String>, output: bool) {
        if self.mutating() {
            return;
        }
        if let Some(pending) = self.pending.take() {
            pending.abort();
        }
        self.mutation = request != Request::Refresh;
        let paths = self.paths.clone();
        let config = self.config.clone();
        self.pending = Some(tokio::spawn(async move {
            execute(&paths, &config, request, selected, output).await
        }));
    }
    /// 【后台任务】【请求完成】只等待已经完成的请求，保持键盘循环响应。
    /// 参数: 无；返回: 已完成的服务结果。
    pub async fn completed(&mut self) -> Option<Result<Update>> {
        self.try_completed()
    }
    /// 【后台任务】【同步采样】仅提取已结束请求，供终端事件循环非阻塞刷新。
    /// 参数: 无；返回: 已完成结果，尚未完成时为空。
    pub fn try_completed(&mut self) -> Option<Result<Update>> {
        if !self.pending.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }
        let pending = self.pending.take().unwrap();
        Some(
            pending
                .now_or_never()?
                .map_err(anyhow::Error::from)
                .and_then(|result| result),
        )
    }
}

impl Drop for TaskService {
    /// 【后台任务】【读取清理】退出管理界面时取消读取，已提交的写操作继续完成。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        if !self.mutation {
            if let Some(pending) = &self.pending {
                pending.abort();
            }
        }
    }
}

/// 【后台任务】【接口适配】执行真实任务操作，再读取任务及输出尾部。
/// 参数: paths/config 为运行环境，request 为操作，selected 为日志目标，output 决定是否读取日志；返回: 最新状态。
async fn execute(
    paths: &SaiPaths,
    config: &AppConfig,
    request: Request,
    selected: Option<String>,
    output: bool,
) -> Result<Update> {
    let notice = match request {
        Request::Refresh => None,
        Request::Stop { id, force } => {
            stop_background_task_for_user(paths, config, &id, force).await?;
            Some(t("Task ended", "任务已结束").to_string())
        }
        Request::Cleanup => {
            cleanup_managed_background_tasks(paths, config, false).await?;
            Some(
                t(
                    "Finished records cleaned; logs kept",
                    "已清理结束记录，日志已保留",
                )
                .to_string(),
            )
        }
    };
    #[derive(Deserialize)]
    struct TaskList {
        tasks: Vec<Task>,
    }
    let mut tasks =
        serde_json::from_str::<TaskList>(&list_managed_background_tasks(paths, config).await?)?
            .tasks;
    tasks.sort_by(|a, b| {
        (b.status == "running")
            .cmp(&(a.status == "running"))
            .then(b.started_at.cmp(&a.started_at))
            .then(a.id.cmp(&b.id))
    });
    let target = selected
        .filter(|id| tasks.iter().any(|task| &task.id == id))
        .or_else(|| tasks.first().map(|task| task.id.clone()));
    let output = if let Some(id) = target.filter(|_| output) {
        let result = async {
            let raw = read_managed_background_output(paths, config, &id, 2000).await?;
            Ok::<Output, anyhow::Error>(serde_json::from_str(&raw)?)
        }
        .await
        .map_err(|error| error.to_string());
        Some((id, result))
    } else {
        None
    };
    Ok(Update {
        tasks,
        output,
        notice,
    })
}
