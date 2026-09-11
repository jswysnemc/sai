#[path = "binary/read_support.rs"]
mod support;
#[path = "binary/read_worker.rs"]
mod worker;

use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use support::*;
use worker::*;

const LIFECYCLE_SOURCE: &str = r#"
    --- 【二进制读取测试】【线程调用】读取、捕获超时或检查仍被占用的预算
    --- @param args table 读取选项及测试动作
    --- @return string|integer 可观察结果
    local function run(args)
        if args.probe then
            for _, action in ipairs({
                function() return sai.binary.read_file("allowed/a") end,
                function() return sai.binary.decode_base64("eA==") end,
            }) do
                local ok, message = pcall(action)
                assert(not ok and tostring(message):find("size limit",1,true), tostring(message))
            end
            return "held"
        end
        if args.catch then
            local ok, message = pcall(sai.binary.read_file,"allowed/a",args.options)
            assert(not ok and tostring(message):find("binary read timed out",1,true), tostring(message))
            return "timeout"
        end
        return sai.binary.read_file("allowed/a",args.options):len()
    end
    sai.register_tool({name="run",description="Read lifecycle",parameters={type="object"},execute=run})
"#;

/// 【二进制读取测试】【线程回收】放行阻塞线程，随后验证同一实例恢复读取
/// @param plugin 待恢复实例；host 为控制读取线程的宿主
/// @returns 无；线程退出前不提前执行恢复断言
async fn release_and_recover(plugin: &sai_plugin_runtime::PluginRuntime, host: &BlockingHost) {
    host.blocking.store(false, Ordering::SeqCst);
    host.worker.release();
    tokio::time::timeout(Duration::from_secs(2), host.worker.released.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin.call_tool("run", json!({}), context()).await.unwrap(),
        "16"
    );
}

/// 【二进制读取测试】【局部时限】过大的请求时限收窄到二进制上限，超时后线程仍占预算
/// @returns 无；Lua 能捕获超时，线程退出后预算恢复
#[tokio::test]
async fn binary_read_timeout_keeps_the_worker_reservation_until_it_exits() {
    let host = BlockingHost::new();
    let plugin = runtime(LIFECYCLE_SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024;
        limits.binary_timeout_ms = 20;
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        plugin.call_tool(
            "run",
            json!({"catch":true,"options":{"timeout_ms":1000000}}),
            context(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, "timeout");
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context())
            .await
            .unwrap(),
        "held"
    );
    assert_eq!(host.calls.load(Ordering::SeqCst), 1);
    release_and_recover(&plugin, &host).await;
}

/// 【二进制读取测试】【外部取消】取消调用不能使尚未结束的读取与下一回调重复占用同一额度
/// @returns 无；其他实例预算独立，原实例在线程退出后恢复
#[tokio::test]
async fn cancelled_read_workers_keep_shared_vm_budget_but_not_other_instance_budgets() {
    let host = BlockingHost::new();
    let plugin = Arc::new(runtime(LIFECYCLE_SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024
    }));
    let instance = plugin.clone();
    let call = tokio::spawn(async move { instance.call_tool("run", json!({}), context()).await });
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context())
            .await
            .unwrap(),
        "held"
    );
    let other = runtime(SOURCE, Arc::new(Host::default()), |limits| {
        limits.binary_bytes = 1024
    });
    assert_eq!(
        other.call_tool("read", json!({}), context()).await.unwrap(),
        "0"
    );
    assert_eq!(host.calls.load(Ordering::SeqCst), 1);
    release_and_recover(&plugin, &host).await;
}

/// 【二进制读取测试】【总时限】文件读取无法扩大外层回调期限，回调超时也保留工作线程预算
/// @returns 无；后续调用不能绕过未归还的预留额度
#[tokio::test]
async fn callback_deadline_remains_effective_during_binary_file_reads() {
    let host = BlockingHost::new();
    let plugin = runtime(LIFECYCLE_SOURCE, host.clone(), |limits| {
        limits.binary_bytes = 1024;
        limits.timeout_ms = 100;
        limits.binary_timeout_ms = 5000;
    });
    let error = plugin
        .call_tool("run", json!({}), context())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    tokio::time::timeout(Duration::from_secs(2), host.worker.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"probe":true}), context())
            .await
            .unwrap(),
        "held"
    );
    release_and_recover(&plugin, &host).await;
}

/// 【二进制读取测试】【实际计费】完成读取归还未用额度，显式关闭能在同一回调继续读取
/// @returns 无；多个缓冲之和受限，重复关闭没有副作用
#[tokio::test]
async fn completed_reads_shrink_reservations_and_closed_buffers_release_their_bytes() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![0; 512];
    let plugin = runtime(
        r#"
        --- 【二进制读取测试】【缓冲共存】验证成功结果按实际长度计费
        --- @return string 验证结果
        local function read()
            local first = sai.binary.read_file("allowed/a")
            local second = sai.binary.read_file("allowed/a")
            assert(first:len()==512 and second:len()==512)
            assert(not pcall(sai.binary.read_file,"allowed/a"))
            first:close(); first:close()
            assert(not pcall(first.len,first))
            local third = sai.binary.read_file("allowed/a")
            assert(third:len()==512)
            return "ok"
        end
        sai.register_tool({name="read",description="Read allocation",parameters={type="object"},execute=read})
    "#,
        host.clone(),
        |limits| limits.binary_bytes = 1024,
    );
    assert_eq!(
        plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(
        host.reads
            .lock()
            .unwrap()
            .iter()
            .map(|read| read.max_bytes)
            .collect::<Vec<_>>(),
        vec![1024, 512, 512]
    );
}

/// 【二进制读取测试】【代次回收】成功和错误退出都撤销保存在 Lua 中的旧文件缓冲
/// @returns 无；旧句柄不可读取，新回调可以用满原预算
#[tokio::test]
async fn success_and_failure_revoke_old_file_buffers_without_waiting_for_gc() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![0; 1024];
    let plugin = runtime(
        r#"
        local saved
        --- 【二进制读取测试】【跨回调句柄】保留 Lua 引用以检查自动撤销
        --- @param args table 是否模拟回调错误
        --- @return integer 新缓冲长度
        local function read(args)
            if saved then
                for _, name in ipairs({"len","bytes","sha256","write","analyze_image"}) do
                    assert(not pcall(saved[name],saved))
                end
                saved:close()
            end
            saved = sai.binary.read_file("allowed/a")
            if args.fail then error("fixture callback failure") end
            return saved:len()
        end
        sai.register_tool({name="read",description="File buffer lifetime",parameters={type="object"},execute=read})
    "#,
        host,
        |limits| limits.binary_bytes = 1024,
    );
    for fail in [false, true, false] {
        let result = plugin
            .call_tool("read", json!({"fail":fail}), context())
            .await;
        if fail {
            assert!(format!("{:#}", result.unwrap_err()).contains("fixture callback failure"));
        } else {
            assert_eq!(result.unwrap(), "1024");
        }
    }
}

/// 【二进制读取测试】【失败归还】读取失败和有界缓冲拒绝超限数据都归还完整预留额度
/// @returns 无；下一次读取可以继续使用同一实例
#[tokio::test]
async fn failed_and_oversized_reads_return_the_entire_reservation() {
    let host = Arc::new(Host::default());
    let plugin = runtime(SOURCE, host.clone(), |limits| limits.binary_bytes = 1024);
    for oversized in [false, true] {
        host.fail.store(!oversized, Ordering::SeqCst);
        *host.body.lock().unwrap() = vec![0; 1025];
        let error = plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(if oversized {
                "size limit"
            } else {
                "fixture read failure"
            }),
            "{error:#}"
        );
        host.fail.store(false, Ordering::SeqCst);
        *host.body.lock().unwrap() = vec![0; 1024];
        assert_eq!(
            plugin
                .call_tool("read", json!({}), context())
                .await
                .unwrap(),
            "1024"
        );
    }
}

/// 【二进制读取测试】【宿主结果校验】宿主不能返回超出请求的已有缓冲，也不能混用其他实例预算
/// @returns 无；拒绝错误结果后本次预留额度全部归还
#[tokio::test]
async fn host_results_cannot_exceed_the_request_or_belong_to_another_vm() {
    let host = Arc::new(Host::default());
    host.retain.store(true, Ordering::SeqCst);
    *host.body.lock().unwrap() = vec![0; 1024];
    let plugin = runtime(SOURCE, host.clone(), |limits| limits.binary_bytes = 2048);
    plugin
        .call_tool("read", json!({}), context())
        .await
        .unwrap();
    host.retain.store(false, Ordering::SeqCst);
    *host.replacement.lock().unwrap() = Some(host.retained.lock().unwrap()[0].clone());
    let error = plugin
        .call_tool("read", json!({"options":{"max_bytes":16}}), context())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("size limit"), "{error:#}");

    let other_host = Arc::new(Host::default());
    *other_host.replacement.lock().unwrap() = Some(host.retained.lock().unwrap()[0].clone());
    let other = runtime(SOURCE, other_host.clone(), |limits| {
        limits.binary_bytes = 1024
    });
    let error = other
        .call_tool("read", json!({}), context())
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("different binary budget"),
        "{error:#}"
    );
    *other_host.body.lock().unwrap() = vec![0; 1024];
    assert_eq!(
        other.call_tool("read", json!({}), context()).await.unwrap(),
        "1024"
    );
    host.retained.lock().unwrap().clear();
    assert_eq!(
        plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap(),
        "1024"
    );
}
