#[path = "binary/support.rs"]
mod support;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use support::*;

const SOURCE: &str = r#"
    sai.register_tool({name='run',description='Lifecycle',parameters={type='object'},execute=function(args)
        local response=sai.binary.request({url='https://example.test',timeout_ms=args.timeout or 10000})
        response.body:close()
        return 'ok'
    end})
"#;

/// 【二进制测试】【请求超时】单次超时丢弃宿主 Future，随后仍可使用同一 VM。
#[tokio::test]
async fn binary_timeouts_release_network_operations_and_allow_recovery() {
    let host = Arc::new(Host::default());
    host.block_network.store(true, Ordering::SeqCst);
    let plugin = runtime(SOURCE, host.clone(), capabilities(), |_| {});
    let error = plugin
        .call_tool("run", json!({"timeout":15}), context(false))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("binary request timed out"));
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
    host.block_network.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "ok"
    );
}

/// 【二进制测试】【外部取消】取消工具调用不能留下仍在运行的网络 Future。
#[tokio::test]
async fn cancellation_drops_the_inflight_binary_request() {
    let host = Arc::new(Host::default());
    host.block_network.store(true, Ordering::SeqCst);
    let plugin = Arc::new(runtime(SOURCE, host.clone(), capabilities(), |_| {}));
    let instance = plugin.clone();
    let task =
        tokio::spawn(async move { instance.call_tool("run", json!({}), context(false)).await });
    tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), host.released.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
    host.block_network.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "ok"
    );
}

/// 【二进制测试】【总时限】大文件请求不能延长插件回调截止时间。
#[tokio::test]
async fn callback_deadline_remains_the_outer_bound() {
    let host = Arc::new(Host::default());
    host.block_network.store(true, Ordering::SeqCst);
    let plugin = runtime(SOURCE, host.clone(), capabilities(), |limits| {
        limits.timeout_ms = 100
    });
    let error = plugin
        .call_tool("run", json!({}), context(false))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"));
    tokio::time::timeout(Duration::from_secs(2), host.released.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
}

/// 【二进制测试】【调用预算】二进制操作消耗统一系统额度，异常不能绕过每回调上限。
#[tokio::test]
async fn binary_operations_consume_the_shared_system_call_budget() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Budget',parameters={type='object'},execute=function()
            local b=sai.binary.request({url='https://example.test'}).body
            local ok,err=pcall(sai.binary.request,{url='https://example.test'})
            assert(not ok and tostring(err):find('system call budget exceeded',1,true))
            b:close(); return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |limits| limits.system_calls = 1,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}
