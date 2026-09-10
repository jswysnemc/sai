mod common;

use common::storage::{capabilities, runtime, StorageHost};
use sai_plugin_runtime::{host::StorageRequest, ExecutionLimits, InvocationContext};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件存储测试】【参数边界】非法键、非 JSON 值和过大记录在进入宿主前拒绝，不消耗调用预算。
#[tokio::test]
async fn invalid_keys_and_values_are_rejected_before_dispatch() {
    let host = Arc::new(StorageHost::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name="check",description="check",access="writes",parameters={type="object"},execute=function()
            local store = sai.storage.plugin
            for _, key in ipairs({"", string.rep("x", 257), "a\nb", "a\0b", string.char(255), 42, true, {}}) do
                for _, action in ipairs({"get", "set", "compare_exchange"}) do
                    assert(not pcall(store[action], key, 1, 2))
                end
            end
            assert(not pcall(store.get))
            local cycle = {}; cycle.self = cycle
            for _, value in ipairs({string.rep("x", 256 * 1024), function() end, cycle}) do
                assert(not pcall(store.set, "key", value))
                assert(not pcall(store.compare_exchange, "key", value, 1))
                assert(not pcall(store.compare_exchange, "key", 1, value))
            end
            store.set(string.rep("x", 256), string.rep("x", 256 * 1024 - 2))
            store.compare_exchange("../arbitrary/key", nil, sai.json.null)
            return "ok"
        end})
    "#,
        capabilities(),
        capabilities(),
        ExecutionLimits {
            system_calls: 2,
            ..Default::default()
        },
        host.clone(),
    );
    assert_eq!(
        plugin
            .call_tool("check", json!({}), writable())
            .await
            .unwrap(),
        "ok"
    );
    let calls = host.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(
        matches!(&calls[0].0, StorageRequest::Set { key, value } if key.len() == 256 && value.as_str().unwrap().len() == 256 * 1024 - 2)
    );
    assert!(
        matches!(&calls[1].0, StorageRequest::CompareExchange { expected, value, .. } if expected.is_null() && value.is_null())
    );
}

/// 【插件存储测试】【共同预算】两种存储与失败宿主调用共用系统额度，下一回调重新获得额度。
#[tokio::test]
async fn scopes_and_host_errors_share_the_per_callback_budget() {
    let host = Arc::new(StorageHost::default());
    host.failing.store(true, Ordering::SeqCst);
    let mut caps = capabilities();
    caps.system.session_storage = true;
    let plugin = runtime(
        r#"
        sai.register_tool({name="check",description="check",parameters={type="object"},execute=function()
            sai.storage.set(42, 1)
            local ok, err = pcall(sai.storage.plugin.get, "key")
            assert(not ok and tostring(err):find("storage fixture failed", 1, true))
            ok, err = pcall(sai.storage.plugin.get, "key")
            assert(not ok and tostring(err):find("system call budget exceeded", 1, true))
            ok, err = pcall(sai.storage.get, "session")
            assert(not ok and tostring(err):find("system call budget exceeded", 1, true))
            return "ok"
        end})
    "#,
        caps.clone(),
        caps,
        ExecutionLimits {
            system_calls: 2,
            ..Default::default()
        },
        host.clone(),
    );
    for count in 1..=2 {
        assert_eq!(
            plugin
                .call_tool("check", json!({}), Default::default())
                .await
                .unwrap(),
            "ok"
        );
        assert_eq!(host.calls.lock().unwrap().len(), count);
        assert_eq!(host.session_calls.load(Ordering::SeqCst), count);
    }
}

/// 【插件存储测试】【有界输出】宿主结果受记录上限和插件输出上限共同限制，错误后实例仍可使用。
#[tokio::test]
async fn oversized_host_output_is_bounded_and_the_runtime_recovers() {
    for limit in [1024, 512 * 1024] {
        let host = Arc::new(StorageHost::default());
        let plugin = runtime(
            r#"
            sai.register_tool({name="read",description="read",parameters={type="object"},execute=function()
                return sai.storage.plugin.get("key")
            end})
        "#,
            capabilities(),
            capabilities(),
            ExecutionLimits {
                output_bytes: limit,
                ..Default::default()
            },
            host.clone(),
        );
        *host.response.lock().unwrap() = json!("x".repeat(limit.min(256 * 1024)));
        let error = plugin
            .call_tool("read", json!({}), Default::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("output exceeds size limit"));
        *host.response.lock().unwrap() = json!("recovered");
        assert_eq!(
            plugin
                .call_tool("read", json!({}), Default::default())
                .await
                .unwrap(),
            "recovered"
        );
    }
}

/// 【插件存储测试】【失效调用】同步读取期间超时或取消后，捕获错误也不能继续调用宿主写入。
#[tokio::test]
async fn timeout_and_cancellation_prevent_later_storage_mutations() {
    use std::time::Duration;
    for cancel in [false, true] {
        let host = Arc::new(StorageHost::default());
        host.delay_ms.store(150, Ordering::SeqCst);
        let plugin = runtime(
            r#"
            sai.register_tool({name="check",description="check",access="writes",parameters={type="object"},execute=function(args)
                if args.slow then
                    pcall(sai.storage.plugin.get, "slow")
                    pcall(sai.storage.plugin.set, "must-not-write", 1)
                end
                return "recovered"
            end})
        "#,
            capabilities(),
            capabilities(),
            ExecutionLimits {
                timeout_ms: if cancel { 2000 } else { 100 },
                ..Default::default()
            },
            host.clone(),
        );
        let cloned = plugin.clone();
        let call = tokio::spawn(async move {
            cloned
                .call_tool("check", json!({"slow":true}), writable())
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
            .await
            .unwrap();
        if cancel {
            call.abort();
            assert!(call.await.unwrap_err().is_cancelled());
        } else {
            assert!(format!("{:#}", call.await.unwrap().unwrap_err()).contains("timed out"));
        }
        host.delay_ms.store(0, Ordering::SeqCst);
        let recovered = tokio::time::timeout(
            Duration::from_secs(2),
            plugin.call_tool("check", json!({}), writable()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(recovered, "recovered");
        assert_eq!(host.calls.lock().unwrap().len(), 1);
    }
}

/// 【插件存储测试】【写入上下文】提供明确写入权限，读写能力继续由工具声明控制。
/// @returns 允许写入的宿主上下文
fn writable() -> InvocationContext {
    InvocationContext {
        allow_writes: true,
        ..Default::default()
    }
}
