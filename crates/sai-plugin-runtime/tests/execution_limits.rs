mod common;

use anyhow::Result;
use async_trait::async_trait;
use common::{manifest, package, RecordingHost};
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// 【插件测试】【指令上限】无限循环必须终止，失败不能污染后续正常调用。
#[tokio::test]
async fn cpu_limit_stops_loops_and_resets_for_the_next_call() {
    let mut package = package(
        r#"
        sai.register_tool({name="spin",description="循环",parameters={type="object"},execute=function() while true do end end})
        sai.register_tool({name="ping",description="存活",parameters={type="object"},execute=function() return "pong" end})
    "#,
    );
    package.manifest.limits.instructions = 5_000;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    let error = plugin
        .call_tool("spin", json!({}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("instruction budget"));
    assert_eq!(
        plugin
            .call_tool("ping", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "pong"
    );
}

/// 【插件测试】【加载限制】入口脚本的循环和大分配也受限制。
#[test]
fn initialization_has_instruction_and_memory_limits() {
    for source in [
        "while true do end",
        "local text=string.rep('x',64*1024*1024)",
    ] {
        let mut package = package(source);
        package.manifest.limits.instructions = 2_000;
        package.manifest.limits.memory_bytes = 1024 * 1024;
        assert!(PluginRuntime::load(
            package,
            json!({}),
            Capabilities::default(),
            Arc::new(RecordingHost::default())
        )
        .is_err());
    }
}

/// 【插件测试】【输出限制】返回值超过约定上限时不得继续交付给宿主。
#[tokio::test]
async fn oversized_result_is_rejected() {
    let mut package=package("sai.register_tool({name='large',description='大结果',parameters={type='object'},execute=function() return string.rep('x',4096) end})");
    package.manifest.limits.output_bytes = 1024;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    assert!(format!(
        "{:#}",
        plugin
            .call_tool("large", json!({}), InvocationContext::default())
            .await
            .unwrap_err()
    )
    .contains("output exceeds"));
}

struct PendingHost {
    entered: Notify,
    dropped: Arc<AtomicBool>,
}

struct DropSignal(Arc<AtomicBool>);
impl Drop for DropSignal {
    /// 【插件测试】【取消观察】记录宿主 Future 已经释放，无参数。
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[async_trait]
impl PluginHost for PendingHost {
    /// 【插件测试】【阻塞 HTTP】模拟等待响应的请求，以观察调用取消。
    /// @param request 请求；capabilities 为网络授权；allow_writes 为调用权限
    /// @returns 永不自行完成的 Future，取消时记录释放状态
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        let _drop = DropSignal(self.dropped.clone());
        self.entered.notify_one();
        std::future::pending().await
    }
}

/// 【插件测试】【取消传播】调用 Future 被取消后必须终止宿主请求并释放插件执行锁。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_a_call_cancels_host_io_and_releases_the_vm() {
    let host = Arc::new(PendingHost {
        entered: Notify::new(),
        dropped: Arc::new(AtomicBool::new(false)),
    });
    let mut manifest = manifest();
    manifest
        .capabilities
        .http
        .insert("https://service.test".into());
    let source = r#"
        sai.register_tool({name="wait",description="等待响应",parameters={type="object"},execute=function() return sai.http.request({url="https://service.test"}).text end})
        sai.register_tool({name="ping",description="存活",parameters={type="object"},execute=function() return "pong" end})
    "#;
    let package = PluginPackage::new(
        manifest.clone(),
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    let plugin =
        PluginRuntime::load(package, json!({}), manifest.capabilities, host.clone()).unwrap();
    let worker = plugin.clone();
    let task = tokio::spawn(async move {
        worker
            .call_tool("wait", json!({}), InvocationContext::default())
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let ping = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool("ping", json!({}), InvocationContext::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(ping, "pong");
    assert!(host.dropped.load(Ordering::Acquire));
}
