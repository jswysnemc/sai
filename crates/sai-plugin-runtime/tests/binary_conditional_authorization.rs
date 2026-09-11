#[path = "binary/conditional_support.rs"]
mod support;

use sai_plugin_runtime::{host::BinaryRevision, Capabilities};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use support::*;

/// 【条件写入测试】【授权交集】读取与写入都须在清单及授权中同时出现
/// @returns 无；任意一侧缺失都不进入宿主
#[tokio::test]
async fn conditional_writes_require_both_declared_and_granted_read_and_write_access() {
    let only_read: Capabilities =
        serde_json::from_value(json!({"system":{"read_paths":["output"]}})).unwrap();
    let only_write: Capabilities =
        serde_json::from_value(json!({"binary":{"write_paths":["output"]}})).unwrap();
    for reduced in [Capabilities::default(), only_read, only_write] {
        for (declared, granted) in [
            (capabilities(), reduced.clone()),
            (reduced.clone(), capabilities()),
        ] {
            let host = Arc::new(Host::default());
            let plugin = load(SOURCE, host.clone(), declared, granted, |_| {}).unwrap();
            let error = plugin
                .call_tool("run", json!({}), context(true))
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("not allowed"), "{error:#}");
            assert!(host.writes.lock().unwrap().is_empty());
        }
    }
}

/// 【条件写入测试】【可信写入权限】可见上下文修改不能覆盖只读调用或只读工具属性
/// @returns 无；权限拒绝发生在宿主执行之前
#[tokio::test]
async fn forged_context_cannot_enable_writes_in_read_only_callbacks() {
    for (source, writable) in [
        (SOURCE.to_string(), false),
        (SOURCE.replace("optional_writes", "read_only"), true),
    ] {
        let host = Arc::new(Host::default());
        let plugin = runtime(&source, host.clone(), |_| {});
        let error = plugin
            .call_tool("run", json!({}), context(writable))
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("read-only"), "{error:#}");
        assert!(host.writes.lock().unwrap().is_empty());
    }
}

/// 【条件写入测试】【条件传递】nil 与摘要是不同条件，宿主接收可信目录、原始字节及完整比较上限
/// @returns 无；匹配结果按布尔值返回
#[tokio::test]
async fn requests_preserve_conditions_bytes_and_trusted_limits() {
    let host = Arc::new(Host::default());
    let plugin = runtime(SOURCE, host.clone(), |limits| limits.binary_bytes = 2048);
    assert_eq!(
        plugin
            .call_tool("run", json!({"data":"\u{0}raw"}), context(true))
            .await
            .unwrap(),
        "false"
    );
    host.matched.store(true, Ordering::SeqCst);
    let digest = "Ab".repeat(32);
    assert_eq!(
        plugin
            .call_tool("run", json!({"expected":digest}), context(true))
            .await
            .unwrap(),
        "true"
    );
    let writes = host.writes.lock().unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].request.expected, BinaryRevision::Missing);
    assert_eq!(
        writes[1].request.expected,
        BinaryRevision::Sha256([0xab; 32])
    );
    assert_eq!(writes[0].bytes, b"\0raw");
    for call in writes.iter() {
        assert_eq!(call.request.max_bytes, 2048);
        assert_eq!(call.request.path, "output/index.json");
        assert_eq!(call.context.workdir, "/trusted");
        assert!(call.context.allow_writes);
        assert_eq!(call.capabilities, capabilities());
    }
}

/// 【条件写入测试】【参数拒绝】路径必须是字符串，摘要必须是严格的 64 位十六进制文本
/// @returns 无；格式错误不触发文件宿主
#[tokio::test]
async fn invalid_paths_and_revision_values_never_reach_the_host() {
    let host = Arc::new(Host::default());
    let plugin = runtime(SOURCE, host.clone(), |_| {});
    for args in [
        json!({"path":42}),
        json!({"path":{}}),
        json!({"path":"../a"}),
        json!({"path":"a\u{0}b"}),
        json!({"expected":false}),
        json!({"expected":42}),
        json!({"expected":{}}),
        json!({"expected":""}),
        json!({"expected":"f".repeat(63)}),
        json!({"expected":"f".repeat(65)}),
        json!({"expected":"g".repeat(64)}),
        json!({"expected":format!("sha256:{}","a".repeat(64))}),
        json!({"expected":"é".repeat(32)}),
    ] {
        assert!(plugin.call_tool("run", args, context(true)).await.is_err());
    }
    assert!(host.writes.lock().unwrap().is_empty());
}

/// 【条件写入测试】【宿主错误计数】条件不匹配和宿主错误都消耗一次调用额度
/// @returns 无；耗尽次数后不再次调用宿主
#[tokio::test]
async fn conflicts_and_host_failures_charge_system_calls() {
    for fail in [false, true] {
        let host = Arc::new(Host::default());
        host.fail.store(fail, Ordering::SeqCst);
        let plugin = runtime(
            r#"
            --- 【条件写入测试】【额度耗尽】同一缓冲最多进入两次条件写入
            --- @return string 校验结果
            local function run()
                local data=sai.binary.from_bytes("a")
                pcall(data.write_if,data,"output/a",nil)
                pcall(data.write_if,data,"output/a",nil)
                local ok,message=pcall(data.write_if,data,"output/a",nil)
                assert(not ok and tostring(message):find("call budget exceeded",1,true),tostring(message))
                return "limited"
            end
            sai.register_tool({name="run",description="Conditional calls",access="writes",parameters={type="object"},execute=run})
        "#,
            host.clone(),
            |limits| limits.system_calls = 3,
        );
        assert_eq!(
            plugin
                .call_tool("run", json!({}), context(true))
                .await
                .unwrap(),
            "limited"
        );
        assert_eq!(host.writes.lock().unwrap().len(), 2);
    }
}
