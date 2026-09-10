use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost, StorageRequest};
use sai_plugin_runtime::{Capabilities, ExecutionLimits, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct StorageHost {
    pub calls: Mutex<Vec<(StorageRequest, bool)>>,
    pub response: Mutex<Value>,
    pub failing: AtomicBool,
    pub delay_ms: AtomicU64,
    pub entered: tokio::sync::Notify,
    pub session_calls: AtomicUsize,
}

#[async_trait]
impl PluginHost for StorageHost {
    /// 【插件存储测试】【调用记录】记录收到的请求与可信权限，提供可控结果或故障。
    /// @param request 键值操作；capabilities 为有效授权；allow_writes 为可信权限
    /// @returns 固定 JSON 结果或注入的宿主错误
    fn plugin_storage(
        &self,
        request: StorageRequest,
        capabilities: &Capabilities,
        allow_writes: bool,
    ) -> Result<Value> {
        assert!(capabilities.system.plugin_storage);
        self.calls.lock().unwrap().push((request, allow_writes));
        let delay = self.delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            self.entered.notify_one();
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        if self.failing.load(Ordering::SeqCst) {
            anyhow::bail!("storage fixture failed");
        }
        Ok(self.response.lock().unwrap().clone())
    }

    /// 【插件存储测试】【旧接口计数】观察会话存储是否与插件存储共享调用预算。
    /// @param request 为操作；session 为会话；capabilities 为有效授权
    /// @returns 空记录
    fn storage(
        &self,
        _request: StorageRequest,
        _session: &str,
        capabilities: &Capabilities,
    ) -> Result<Value> {
        assert!(capabilities.system.session_storage);
        self.session_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Value::Null)
    }

    /// 【插件存储测试】【网络隔离】存储测试不提供外部网络。
    /// @param request 请求；capabilities 为授权；allow_writes 为可信权限
    /// @returns 固定不可用错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        anyhow::bail!("HTTP is unavailable in the storage fixture")
    }
}

/// 【插件存储测试】【能力样本】只声明插件私有持久化能力。
/// @returns 最小跨会话存储授权
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"plugin_storage":true}})).unwrap()
}

/// 【插件存储测试】【运行时样本】分别注入清单、授权与预算，验证正式绑定。
/// @param source 为源码；declared 为声明；granted 为授权；limits 为预算；host 为测试宿主
/// @returns 加载完成的运行时
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
