use super::binary_files::{context, SOURCE};
use crate::plugins::host::SaiPluginHost;
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

struct SignalledHost {
    entered: tokio::sync::Notify,
}

#[async_trait]
impl PluginHost for SignalledHost {
    /// 【写入取消测试】【文本隔离】当前脚本没有 HTTP 调用。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 明确错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【写入取消测试】【调度观测】真实宿主把阻塞写入加入队列后通知测试取消。
    /// @param path 路径；data 为缓冲；context 为可信目录；capabilities 为授权
    /// @returns 实际写入结果
    async fn write_binary(
        &self,
        path: String,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryFile> {
        use std::future::Future;
        let operation = SaiPluginHost.write_binary(path, data, context, capabilities);
        tokio::pin!(operation);
        std::future::poll_fn(|cx| {
            let result = operation.as_mut().poll(cx);
            if result.is_pending() {
                self.entered.notify_one();
            }
            result
        })
        .await
    }
}

/// 【写入取消测试】【发布边界】取消已经排队的真实写入后，工作线程不创建目录且 VM 可以继续使用。
#[test]
fn cancelled_queued_writes_never_create_or_publish_output() {
    // 1. 【写入取消测试】【确定性队列】Lua 占用唯一阻塞线程，文件工作必须等待 Lua 释放后才执行
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let root=tempfile::tempdir().unwrap();
        let host=Arc::new(SignalledHost {entered:tokio::sync::Notify::new()});
        let manifest=PluginManifest::parse(&json!({"api_version":1,"id":"cancel-write","version":"1.0.0","name":"Cancel",
            "description":"Cancel actual file writes","entry":"init.lua","capabilities":{"binary":{"write_paths":["output"]}}}).to_string()).unwrap();
        let grants=manifest.capabilities.clone();
        let package=PluginPackage::new(manifest,[("init.lua".into(),SOURCE.into())].into()).unwrap();
        let plugin=Arc::new(PluginRuntime::load(package,json!({}),grants,host.clone()).unwrap());
        let instance=plugin.clone();let invocation=context(root.path());
        let call=tokio::spawn(async move { instance.call_tool("write",json!({"path":"output/image.png"}),invocation).await });
        tokio::time::timeout(std::time::Duration::from_secs(2),host.entered.notified()).await.unwrap();
        call.abort();assert!(call.await.unwrap_err().is_cancelled());
        let result=tokio::time::timeout(std::time::Duration::from_secs(2),plugin.call_tool("write",json!({"probe":true}),context(root.path()))).await.unwrap().unwrap();
        assert_eq!(result,"idle");assert!(!root.path().join("output").exists());
    });
}
