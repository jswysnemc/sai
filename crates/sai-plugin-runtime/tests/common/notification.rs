use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{
    HttpRequest, HttpResponse, NotificationDelivery, NotificationRequest, PluginHost, SystemContext,
};
use sai_plugin_runtime::{Capabilities, ExecutionLimits, PluginPackage, PluginRuntime};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
pub struct NotificationHost {
    pub calls: Mutex<Vec<(NotificationRequest, SystemContext)>>,
    pub pending: AtomicBool,
    pub inconsistent: AtomicBool,
    pub active: AtomicUsize,
    pub entered: Notify,
    pub released: Notify,
}

struct Lease<'a>(&'a NotificationHost);

impl Drop for Lease<'_> {
    /// 【通知测试】【释放观察】回调取消时减少活动数量并通知等待方。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

#[async_trait]
impl PluginHost for NotificationHost {
    /// 【通知测试】【投递捕获】记录已授权请求，模拟宿主错误或可取消的等待。
    /// @param request 请求；context 为可信目录与权限；capabilities 为授权
    /// @returns 请求对应的成功结果或测试错误
    async fn notify(
        &self,
        request: NotificationRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<NotificationDelivery> {
        assert!(capabilities.system.notify && context.allow_writes);
        self.calls.lock().unwrap().push((request.clone(), context));
        self.active.fetch_add(1, Ordering::SeqCst);
        let _lease = Lease(self);
        self.entered.notify_one();
        if self.pending.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if request.title == "fail" {
            anyhow::bail!("notification fixture failed");
        }
        Ok(NotificationDelivery {
            desktop: request.desktop != self.inconsistent.load(Ordering::SeqCst),
            sound: request.sound.is_some(),
        })
    }

    /// 【通知测试】【网络隔离】通知测试不提供网络。
    /// @param request 请求；capabilities 为授权；allow_writes 为调用权限
    /// @returns 固定不可用错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        anyhow::bail!("HTTP is unavailable in notification tests")
    }
}

/// 【通知测试】【最小授权】只允许主动通知，不继承答复策略权限。
/// @returns 通知投递能力
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"notify":true}})).unwrap()
}

/// 【通知测试】【运行时】分别指定声明、授权和限制，通过正式入口加载源码。
/// @param source 源码；declared 为声明；granted 为授权；limits 为限制；host 为宿主
/// @returns 完整运行时
pub fn runtime(
    source: &str,
    declared: Capabilities,
    granted: Capabilities,
    limits: ExecutionLimits,
    host: Arc<dyn PluginHost>,
) -> PluginRuntime {
    let mut manifest = super::manifest();
    manifest.capabilities = declared;
    manifest.limits = limits;
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    PluginRuntime::load(package, json!({}), granted, host).unwrap()
}
