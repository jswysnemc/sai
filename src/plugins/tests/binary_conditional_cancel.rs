use super::binary_conditional_support::*;
use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities};
use serde_json::json;
use std::{sync::Arc, time::Duration};

struct QueuedHost {
    actual: PrivatePluginHost,
    entered: tokio::sync::Notify,
}

#[async_trait]
impl PluginHost for QueuedHost {
    /// 【条件取消测试】【网络隔离】文件测试不能执行网络请求
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【条件取消测试】【排队观测】通知真实宿主已经提交文件工作
    /// @param request 条件；data 为缓冲；context 为可信目录；capabilities 为授权
    /// @returns 真实条件写入结果
    async fn write_binary_if(
        &self,
        request: BinaryConditionalWrite,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<bool> {
        use std::future::Future;
        let operation = self
            .actual
            .write_binary_if(request, data, context, capabilities);
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

/// 【条件取消测试】【排队撤销】取消尚未运行的实际文件工作后不创建目录且预算恢复
/// @returns 无；使用唯一阻塞线程确定工作顺序
#[test]
fn cancelled_queued_conditional_writes_do_not_create_output_and_release_budget() {
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    executor.block_on(async {
        let root=tempfile::tempdir().unwrap();
        let host=Arc::new(QueuedHost {actual:PrivatePluginHost::new(&SaiPaths::for_tests(root.path()),"conditional-files"),entered:tokio::sync::Notify::new()});
        let source=r#"
            --- 【条件取消测试】【恢复探测】取消后使用全部预算构造新缓冲
            --- @param args table 是否只检查预算
            --- @return integer|boolean 缓冲长度或条件结果
            local function run(args)
                local data=sai.binary.from_bytes(string.rep("x",1024))
                if args.probe then return data:len() end
                return data:write_if("output/a",nil)
            end
            sai.register_tool({name="run",description="Queued condition",access="writes",parameters={type="object"},execute=run})
        "#;
        let plugin=Arc::new(with_host(host.clone(),&["output"],&["output"],source,|manifest| manifest.limits.binary_bytes=1024));
        let instance=plugin.clone();let invocation=context(root.path());
        let call=tokio::spawn(async move {instance.call_tool("run",json!({}),invocation).await});
        tokio::time::timeout(Duration::from_secs(2),host.entered.notified()).await.unwrap();
        call.abort();assert!(call.await.unwrap_err().is_cancelled());
        let output=tokio::time::timeout(Duration::from_secs(2),plugin.call_tool("run",json!({"probe":true}),context(root.path()))).await.unwrap().unwrap();
        assert_eq!(output,"1024");
        assert!(!root.path().join("output").exists());
        assert!(!root.path().join("state").exists());
    });
}

/// 【条件取消测试】【锁等待时限】普通与条件写入共享正式锁，等待超时不能创建输出
/// @returns 无；释放锁后同一运行时可以继续正常发布
#[tokio::test]
async fn both_ordinary_and_conditional_writes_wait_on_the_same_lock_and_cancel_cleanly() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.state_dir.join("plugin-binary-write.lock"))
        .unwrap();
    lock.lock().unwrap();
    let plugin = configured(root.path(), &["output"], &["output"], |manifest| {
        manifest.limits.binary_timeout_ms = 40
    });
    for ordinary in [true, false] {
        let error = plugin
            .call_tool(
                "run",
                json!({"path":"output/a","ordinary":ordinary}),
                context(root.path()),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("timed out"), "{error:#}");
        assert!(!root.path().join("output").exists());
    }
    lock.unlock().unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"path":"output/a"}), context(root.path()))
            .await
            .unwrap(),
        "true"
    );
    assert_eq!(std::fs::read(root.path().join("output/a")).unwrap(), b"new");
}
