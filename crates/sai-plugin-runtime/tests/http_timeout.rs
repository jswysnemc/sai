mod common;

use anyhow::Result;
use async_trait::async_trait;
use common::{package, RecordingHost};
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct PendingHost {
    requests: Mutex<Vec<HttpRequest>>,
    dropped: Arc<AtomicUsize>,
}

/// 【插件测试】【超时范围】缺省、零值和超大值在宿主调用前归一化，负值被拒绝。
#[tokio::test]
async fn request_timeouts_are_defaulted_and_clamped_before_host_dispatch() {
    let host = Arc::new(RecordingHost::default());
    let mut package = package(
        r#"
        sai.register_tool({
            name="timeout", description="Observe effective request deadlines",
            parameters={type="object",properties={timeout_ms={type="integer"}}},
            execute=function(args)
                return sai.http.request({url="https://example.test/",timeout_ms=args.timeout_ms}).text
            end,
        })
    "#,
    );
    package
        .manifest
        .capabilities
        .http
        .insert("https://example.test".into());
    let grants = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    for args in [
        json!({}),
        json!({"timeout_ms":0}),
        json!({"timeout_ms":90000}),
    ] {
        plugin
            .call_tool("timeout", args, InvocationContext::default())
            .await
            .unwrap();
    }
    assert!(plugin
        .call_tool(
            "timeout",
            json!({"timeout_ms":-1}),
            InvocationContext::default()
        )
        .await
        .is_err());
    let timeouts: Vec<_> = host
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| request.timeout_ms)
        .collect();
    assert_eq!(timeouts, vec![30000, 1, 30000]);
}

struct PendingCall(Arc<AtomicUsize>);

/// 【插件测试】【显式长查询】包可在硬上限内声明较长请求，缺省限制不会随之改变。
#[tokio::test]
async fn explicitly_declared_http_timeouts_preserve_long_search_requests() {
    let mut package = package(
        r#"
        sai.register_tool({name="query",description="Long query",parameters={type="object"},execute=function(args)
            return sai.http.request({url="https://example.test/", timeout_ms=args.timeout_ms}).text
        end})
    "#,
    );
    package
        .manifest
        .capabilities
        .http
        .insert("https://example.test".into());
    package.manifest.limits.timeout_ms = 750_000;
    package.manifest.limits.http_timeout_ms = 120_000;
    let host = Arc::new(RecordingHost::default());
    let granted = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package.clone(), json!({}), granted, host.clone()).unwrap();
    for timeout in [120_000, 150_000] {
        plugin
            .call_tool(
                "query",
                json!({"timeout_ms":timeout}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
    }
    assert!(host
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|request| request.timeout_ms == 120_000));
    package.manifest.limits.http_timeout_ms = 120_001;
    assert!(package.manifest.validate().is_err());
    package.manifest.limits.http_timeout_ms = 120_000;
    package.manifest.limits.timeout_ms = 900_001;
    assert!(package.manifest.validate().is_err());
}

impl Drop for PendingCall {
    /// 【插件测试】【请求回收】记录未完成的宿主 Future 何时释放，无返回值。
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl PluginHost for PendingHost {
    /// 【插件测试】【延迟请求】记录请求后持续等待，验证运行时主动回收 I/O。
    /// @param request 已授权请求；capabilities 为网络授权；allow_writes 为调用权限
    /// @returns 持续等待，由调用方取消 Future
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        assert_eq!(capabilities.http, ["https://example.test".into()].into());
        assert!(!allow_writes);
        self.requests.lock().unwrap().push(request);
        let _pending = PendingCall(self.dropped.clone());
        std::future::pending().await
    }
}

/// 【插件测试】【请求超时】单请求超时可被 Lua 捕获，随后仍能返回静态结果并再次调用。
#[tokio::test]
async fn request_timeout_releases_io_and_preserves_callback_fallback() {
    let host = Arc::new(PendingHost::default());
    let mut package = package(
        r#"
        sai.register_tool({
            name="fallback", description="Keep static data when the HTTP request times out",
            parameters={type="object"},
            execute=function()
                local ok, result = pcall(sai.http.request, {
                    url="https://example.test/", timeout_ms=10,
                })
                return {ok=ok, detail=tostring(result), rules={"static rule"}}
            end,
        })
    "#,
    );
    package
        .manifest
        .capabilities
        .http
        .insert("https://example.test".into());
    let grants = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    for count in 1..=2 {
        let output = tokio::time::timeout(
            Duration::from_secs(3),
            plugin.call_tool("fallback", json!({}), InvocationContext::default()),
        )
        .await
        .expect("request timeout did not finish before the callback deadline")
        .unwrap();
        let result: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(result["ok"], false);
        assert!(result["detail"]
            .as_str()
            .unwrap()
            .contains("HTTP request timed out"));
        assert_eq!(result["rules"], json!(["static rule"]));
        assert_eq!(host.requests.lock().unwrap().len(), count);
        assert_eq!(host.dropped.load(Ordering::SeqCst), count);
    }
}
