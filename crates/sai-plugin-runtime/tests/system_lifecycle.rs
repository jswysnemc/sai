mod common;

use common::system::{capabilities, runtime, SystemHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const SOURCE: &str = r#"
    sai.register_tool({name='run',description='Process lifetime',parameters={type='object'},execute=function(args)
        return sai.process.output('read', {}, {timeout_ms=args.timeout or 10000})
    end})
"#;

/// 【系统接口测试】【进程超时】单次超时释放宿主 Future，返回超时标记后允许再次调用。
#[tokio::test]
async fn process_timeout_drops_the_host_future_and_the_runtime_recovers() {
    let host = Arc::new(SystemHost::default());
    host.blocking.store(true, Ordering::SeqCst);
    let plugin = runtime(SOURCE, host.clone(), capabilities(), |_| {});
    let output = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool("run", json!({"timeout":15}), InvocationContext::default()),
    )
    .await
    .unwrap()
    .unwrap();
    let output: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["timed_out"], true);
    assert_eq!(output["status"], json!(null));
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
    host.blocking.store(false, Ordering::SeqCst);
    let output = plugin
        .call_tool("run", json!({}), InvocationContext::default())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output).unwrap()["status"],
        0
    );
}

/// 【系统接口测试】【外部取消】取消真实调用后宿主操作不残留，不能延续到下一轮。
#[tokio::test]
async fn cancelling_a_call_releases_the_inflight_system_operation() {
    let host = Arc::new(SystemHost::default());
    host.blocking.store(true, Ordering::SeqCst);
    let plugin = Arc::new(runtime(SOURCE, host.clone(), capabilities(), |_| {}));
    let instance = plugin.clone();
    let call = tokio::spawn(async move {
        instance
            .call_tool("run", json!({}), InvocationContext::default())
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), host.entered.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 1);
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), host.released.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
    host.blocking.store(false, Ordering::SeqCst);
    assert!(plugin
        .call_tool("run", json!({}), InvocationContext::default())
        .await
        .is_ok());
}

/// 【系统接口测试】【总时限】极大单次超时不能延长回调期限，超时后也不能保留宿主操作。
#[tokio::test]
async fn process_options_cannot_extend_the_callback_deadline() {
    let host = Arc::new(SystemHost::default());
    host.blocking.store(true, Ordering::SeqCst);
    let plugin = runtime(SOURCE, host.clone(), capabilities(), |limits| {
        limits.timeout_ms = 100
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool(
            "run",
            json!({"timeout":i64::MAX}),
            InvocationContext::default(),
        ),
    )
    .await
    .unwrap();
    match result {
        Ok(output) => assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).unwrap()["timed_out"],
            true
        ),
        Err(error) => assert!(format!("{error:#}").contains("timed out")),
    }
    tokio::time::timeout(Duration::from_secs(2), host.released.notified())
        .await
        .unwrap();
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
}

/// 【系统接口测试】【失败计数】实际进入宿主但超时的调用仍消耗系统预算。
#[tokio::test]
async fn a_timed_out_process_consumes_its_system_call_budget() {
    let host = Arc::new(SystemHost::default());
    host.blocking.store(true, Ordering::SeqCst);
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Timeout budget',parameters={type='object'},execute=function()
            assert(sai.process.output('read', {}, {timeout_ms=15}).timed_out)
            local ok,err=pcall(sai.process.output,'read',{})
            assert(not ok and tostring(err):find('system call budget exceeded',1,true))
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |limits| limits.system_calls = 1,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(host.calls.lock().unwrap().len(), 1);
    assert_eq!(host.active.load(Ordering::SeqCst), 0);
}
