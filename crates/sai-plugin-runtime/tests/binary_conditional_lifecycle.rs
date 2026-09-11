#[path = "binary/conditional_support.rs"]
mod support;
#[path = "binary/conditional_worker.rs"]
mod worker;

use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use support::*;
use worker::*;

const SOURCE: &str = r#"
    --- 【条件写入测试】【取消预算】捕获局部超时或检查线程仍持有的全部预算
    --- @param args table 是否探测预算或捕获超时
    --- @return string|boolean 可观察结果
    local function run(args)
        if args.probe then
            local ok,message=pcall(sai.binary.from_bytes,"x")
            assert(not ok and tostring(message):find("size limit",1,true),tostring(message))
            return "held"
        end
        local data=sai.binary.from_bytes(string.rep("x",1024))
        if args.catch then
            local ok,message=pcall(data.write_if,data,"output/a",nil)
            assert(not ok and tostring(message):find("timed out",1,true),tostring(message))
            data:close()
            return "timeout"
        end
        return data:write_if("output/a",nil)
    end
    sai.register_tool({name="run",description="Conditional lifetime",access="writes",parameters={type="object"},execute=run})
"#;

/// 【条件写入测试】【预算恢复】工作线程退出后同一虚拟机重新获得完整容量
/// @param plugin 原实例；host 为阻塞宿主
/// @returns 无；先确认线程已释放，再进行恢复断言
async fn recover(plugin: &sai_plugin_runtime::PluginRuntime, host: &BlockingHost) {
    host.blocking.store(false, Ordering::SeqCst);
    host.worker.release();
    tokio::time::timeout(Duration::from_secs(2), host.worker.released.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "true"
    );
}

/// 【条件写入测试】【局部超时】单次时限可以捕获，但关闭句柄不能释放实际线程持有的数据
/// @returns 无；线程放行前新回调不能再次用满预算
#[tokio::test]
async fn conditional_write_timeout_keeps_worker_budget_until_actual_completion() {
    let host = BlockingHost::new();
    let plugin = runtime(SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024;
        limits.binary_timeout_ms = 40;
    });
    assert_eq!(
        plugin
            .call_tool("run", json!({"catch":true}), context(true))
            .await
            .unwrap(),
        "timeout"
    );
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context(true))
            .await
            .unwrap(),
        "held"
    );
    recover(&plugin, &host).await;
}

/// 【条件写入测试】【外部取消】取消回调后预算随实际线程保留，其他 VM 不受影响
/// @returns 无；原实例在线程完成后恢复
#[tokio::test]
async fn externally_cancelled_writes_retain_their_lease_but_do_not_block_other_vms() {
    let host = BlockingHost::new();
    let plugin = Arc::new(runtime(SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024
    }));
    let instance = plugin.clone();
    let call =
        tokio::spawn(async move { instance.call_tool("run", json!({}), context(true)).await });
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context(true))
            .await
            .unwrap(),
        "held"
    );
    let other = runtime(SOURCE, Arc::new(Host::default()), |limits| {
        limits.binary_bytes = 1024
    });
    assert_eq!(
        other
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "false"
    );
    recover(&plugin, &host).await;
}

/// 【条件写入测试】【回调总时限】较长二进制时限不能扩大外层回调期限
/// @returns 无；外层超时同样保留实际线程租约
#[tokio::test]
async fn conditional_writes_cannot_extend_callback_deadlines() {
    let host = BlockingHost::new();
    let plugin = runtime(SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024;
        limits.timeout_ms = 150;
        limits.binary_timeout_ms = 5000;
    });
    let error = plugin
        .call_tool("run", json!({}), context(true))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context(true))
            .await
            .unwrap(),
        "held"
    );
    recover(&plugin, &host).await;
}
