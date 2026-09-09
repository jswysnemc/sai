mod common;

use common::services::{context, runtime, RecordingServices};
use sai_plugin_runtime::Capabilities;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// 【插件测试】【取消回收】取消真实模型或工具 Future 后，同一 Lua 实例仍可继续调用。
#[tokio::test]
async fn cancellation_releases_model_and_tool_futures() {
    for model in [true, false] {
        let grants: Capabilities =
            serde_json::from_value(json!({"model":true,"tools":["read"]})).unwrap();
        let plugin = runtime(
            r#"
            sai.register_tool({name="run",description="Cancellable service",parameters={type="object"},execute=function(args)
                if args.model then
                    return sai.model.complete({messages={{role="user",content="question"}}}).content
                end
                return sai.tools.call("read",{})
            end})
        "#,
            grants.clone(),
            grants,
            |_| {},
        );
        let services = Arc::new(RecordingServices::default());
        services.block_model.store(model, Ordering::SeqCst);
        services.block_tool.store(!model, Ordering::SeqCst);
        let running_plugin = plugin.clone();
        let running_context = context(services.clone(), false);
        let task = tokio::spawn(async move {
            running_plugin
                .call_tool("run", json!({"model":model}), running_context)
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), services.entered.notified())
            .await
            .unwrap();
        assert_eq!(services.active.load(Ordering::SeqCst), 1);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(2), services.released.notified())
            .await
            .unwrap();
        assert_eq!(services.active.load(Ordering::SeqCst), 0);
        services.block_model.store(false, Ordering::SeqCst);
        services.block_tool.store(false, Ordering::SeqCst);
        let output = plugin
            .call_tool("run", json!({"model":model}), context(services, false))
            .await
            .unwrap();
        assert!(output.ends_with("result"));
    }
}

/// 【插件测试】【请求超时】模型请求超时能被 Lua 捕获，并立即释放宿主 I/O。
#[tokio::test]
async fn model_timeout_is_catchable_and_releases_the_request() {
    let grants: Capabilities = serde_json::from_value(json!({"model":true})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Timeout",parameters={type="object"},execute=function()
            local ok, err = pcall(sai.model.complete,{messages={{role="user",content="question"}},timeout_ms=15})
            assert(not ok)
            return tostring(err)
        end})
    "#,
        grants.clone(),
        grants,
        |_| {},
    );
    let services = Arc::new(RecordingServices::default());
    services.block_model.store(true, Ordering::SeqCst);
    let output = plugin
        .call_tool("run", json!({}), context(services.clone(), false))
        .await
        .unwrap();
    assert!(output.contains("model request timed out"));
    assert_eq!(services.active.load(Ordering::SeqCst), 0);
}
