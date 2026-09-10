use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{
    HttpRequest, HttpResponse, PluginHost, ScheduledStatus, ScheduledTask, SchedulerRequest,
    SchedulerResponse, SystemContext,
};
use sai_plugin_runtime::{Capabilities, ExecutionLimits, PluginRuntime};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

pub const ID: &str = "job-0123456789abcdef0123456789abcdef";

#[derive(Default)]
pub struct SchedulerHost {
    pub calls: Mutex<Vec<(SchedulerRequest, SystemContext)>>,
    pub pending: AtomicBool,
    pub active: AtomicUsize,
    pub entered: Notify,
    pub released: Notify,
}

struct Lease<'a>(&'a SchedulerHost);

impl Drop for Lease<'_> {
    /// 【调度测试】【释放观察】取消等待时记录宿主 Future 已经退出。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

#[async_trait]
impl PluginHost for SchedulerHost {
    /// 【调度测试】【操作捕获】模拟成功和错误结果，保存可信工作目录及权限。
    /// @param request 操作；context 为可信上下文；capabilities 为有效授权
    /// @returns 请求对应的固定结果
    async fn scheduler(
        &self,
        request: SchedulerRequest,
        context: &SystemContext,
        capabilities: &Capabilities,
    ) -> Result<SchedulerResponse> {
        request.authorize(capabilities, context.allow_writes)?;
        self.calls
            .lock()
            .unwrap()
            .push((request.clone(), context.clone()));
        self.active.fetch_add(1, Ordering::SeqCst);
        let _lease = Lease(self);
        self.entered.notify_one();
        if self.pending.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(match request {
            SchedulerRequest::Schedule(request) => {
                if request.command == "fail" {
                    anyhow::bail!("scheduler fixture failed");
                }
                if request.command == "inconsistent" {
                    return Ok(SchedulerResponse::Changed(true));
                }
                SchedulerResponse::Task(Some(ScheduledTask {
                    id: ID.into(),
                    command: request.command,
                    arguments: request.arguments,
                    due_at: request.due_at,
                    status: ScheduledStatus::Scheduled,
                    pid: Some(1234),
                    created_at: 1,
                    finished_at: None,
                    output: None,
                    output_truncated: false,
                    error: None,
                }))
            }
            SchedulerRequest::List(_) => SchedulerResponse::Tasks(Vec::new()),
            SchedulerRequest::Get(_) | SchedulerRequest::Resume(_) => SchedulerResponse::Task(None),
            SchedulerRequest::Cancel(_) => SchedulerResponse::Changed(false),
        })
    }

    /// 【调度测试】【网络隔离】调度契约测试不提供网络。
    /// @param request 请求；capabilities 为授权；allow_writes 为可信权限
    /// @returns 固定不可用错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        anyhow::bail!("HTTP is unavailable in scheduler tests")
    }
}

/// 【调度测试】【最小授权】仅授予后台调度能力。
/// @returns 独立调度授权
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"schedule":true}})).unwrap()
}

/// 【调度测试】【完整实例】使用正式加载入口与可替换宿主。
/// @param source 源码；granted 为授权；limits 为限制；host 为捕获宿主
/// @returns 可执行运行时
pub fn runtime(
    source: &str,
    granted: Capabilities,
    limits: ExecutionLimits,
    host: Arc<dyn PluginHost>,
) -> PluginRuntime {
    let mut package = super::package(source);
    package.manifest.capabilities = capabilities();
    package.manifest.limits = limits;
    PluginRuntime::load(package, json!({}), granted, host).unwrap()
}
