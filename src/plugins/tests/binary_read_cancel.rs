use super::binary_read_support::context;
use crate::plugins::host::SaiPluginHost;
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

struct QueuedHost {
    entered: tokio::sync::Notify,
}

#[async_trait]
impl PluginHost for QueuedHost {
    /// 【二进制读取测试】【网络隔离】取消测试不访问网络
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【二进制读取测试】【实际队列】实际宿主将文件读取加入阻塞队列后通知测试取消
    /// @param path 路径；buffer 为预算；context 为可信目录；capabilities 为授权
    /// @returns 实际宿主读取结果
    async fn read_binary(
        &self,
        path: String,
        buffer: BinaryReadBuffer,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryData> {
        use std::future::Future;
        let operation = SaiPluginHost.read_binary(path, buffer, context, capabilities);
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

/// 【二进制读取测试】【排队取消】取消已经排队的实际读取后，虚拟机和全部字节预算可以继续使用
/// @returns 无；断言依赖固定阻塞队列顺序，不依赖磁盘读取速度
#[test]
fn cancelled_queued_file_reads_release_budget_and_leave_the_vm_usable() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("image.bin"), vec![0;1024]).unwrap();
        let host = Arc::new(QueuedHost {entered:tokio::sync::Notify::new()});
        let manifest = PluginManifest::parse(&json!({
            "api_version":1,"id":"cancel-read","version":"1.0.0","name":"Cancel read",
            "description":"Cancel real queued read","entry":"init.lua",
            "capabilities":{"system":{"read_paths":["."]}},"limits":{"binary_bytes":1024},
        }).to_string()).unwrap();
        let grants = manifest.capabilities.clone();
        let source = r#"
            --- 【二进制读取测试】【排队调用】读取取消后用纯计算检查全部预算是否恢复
            --- @param args table 是否执行恢复探测
            --- @return integer 原始字节数
            local function read(args)
                if args.probe then
                    return sai.binary.decode_base64(string.rep("A",1364).."AA=="):len()
                end
                return sai.binary.read_file("image.bin"):len()
            end
            sai.register_tool({name="read",description="Queued file read",parameters={type="object"},execute=read})
        "#;
        let package = PluginPackage::new(manifest, [("init.lua".into(),source.into())].into()).unwrap();
        let plugin = Arc::new(PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap());
        let instance = plugin.clone();
        let invocation = context(root.path());
        // 1. 【二进制读取测试】【唯一线程】Lua 持有唯一阻塞线程，实际文件读取必须排队
        let call = tokio::spawn(async move { instance.call_tool("read", json!({}), invocation).await });
        tokio::time::timeout(Duration::from_secs(2), host.entered.notified()).await.unwrap();
        call.abort();
        assert!(call.await.unwrap_err().is_cancelled());
        let output = tokio::time::timeout(Duration::from_secs(2), plugin.call_tool(
            "read", json!({"probe":true}), context(root.path()),
        )).await.unwrap().unwrap();
        assert_eq!(output, "1024");
    });
}
