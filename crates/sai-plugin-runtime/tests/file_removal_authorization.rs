#[path = "file_removal/support.rs"]
mod support;

use sai_plugin_runtime::{host::FileRemovalKind, Capabilities};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use support::*;

/// 【删除绑定测试】【权限交集】旧权限及另一删除类型均不能打开当前接口
/// @returns 无；声明或实际授权缺失时均不进入宿主
#[tokio::test]
async fn each_removal_kind_requires_matching_declared_and_granted_capability() {
    for (operation, other) in [
        ("remove_file", "trash_paths"),
        ("trash_file", "remove_paths"),
    ] {
        for reduced in [
            Capabilities::default(),
            serde_json::from_value(json!({"system":{other:["output"]}})).unwrap(),
            serde_json::from_value(
                json!({"system":{"read_paths":["output"]},"binary":{"write_paths":["output"]}}),
            )
            .unwrap(),
        ] {
            for (declared, granted) in [
                (capabilities(), reduced.clone()),
                (reduced.clone(), capabilities()),
            ] {
                let host = Arc::new(Host::default());
                let plugin = load(SOURCE, host.clone(), declared, granted, |_| {}).unwrap();
                let error = plugin
                    .call_tool("run", json!({"operation":operation}), context(true))
                    .await
                    .unwrap_err();
                assert!(format!("{error:#}").contains("not allowed"), "{error:#}");
                assert!(host.calls.lock().unwrap().is_empty());
            }
        }
    }
}

/// 【删除绑定测试】【真实权限】修改 Lua 上下文不能扩大调用权限或工具的只读属性
/// @returns 无；两种删除均在宿主调用前拒绝
#[tokio::test]
async fn visible_context_cannot_override_read_only_invocations_or_tools() {
    for (source, writable) in [
        (SOURCE.to_string(), false),
        (SOURCE.replace("optional_writes", "read_only"), true),
    ] {
        let host = Arc::new(Host::default());
        let plugin = runtime(&source, host.clone(), |_| {});
        for operation in ["remove_file", "trash_file"] {
            let error = plugin
                .call_tool("run", json!({"operation":operation}), context(writable))
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("read-only"), "{error:#}");
        }
        assert!(host.calls.lock().unwrap().is_empty());
    }
}

/// 【删除绑定测试】【原始请求】宿主收到明确的操作类型、原路径及可信目录
/// @returns 无；成功与缺失结果保持布尔值
#[tokio::test]
async fn removal_requests_preserve_kind_path_and_trusted_context() {
    let host = Arc::new(Host::default());
    let plugin = runtime(SOURCE, host.clone(), |_| {});
    for (operation, result) in [("remove_file", false), ("trash_file", true)] {
        host.result.store(result, Ordering::SeqCst);
        assert_eq!(
            plugin
                .call_tool(
                    "run",
                    json!({"operation":operation,"path":"output/图片~.png"}),
                    context(true)
                )
                .await
                .unwrap(),
            result.to_string()
        );
    }
    let calls = host.calls.lock().unwrap();
    assert_eq!(calls[0].request.kind, FileRemovalKind::Permanent);
    assert_eq!(calls[1].request.kind, FileRemovalKind::Trash);
    for call in calls.iter() {
        assert_eq!(call.request.path, "output/图片~.png");
        assert_eq!(call.context.workdir, "/trusted");
        assert!(call.context.allow_writes);
        assert_eq!(call.capabilities, capabilities());
    }
}

/// 【删除绑定测试】【严格参数】拒绝缺失值、多余参数、隐式字符串转换和非法路径
/// @returns 无；参数失败不进入宿主也不消耗调用次数
#[tokio::test]
async fn invalid_removal_arguments_never_reach_the_host_or_consume_budget() {
    let host = Arc::new(Host::default());
    let source = r#"
        --- 【删除绑定测试】【参数拒绝】检查两个单参数接口
        --- @return string 检查结果
        local function run()
            for _,name in ipairs({"remove_file","trash_file"}) do
                local operation=sai.fs[name]
                assert(not pcall(operation))
                assert(not pcall(operation,"output/a",true))
                for _,path in ipairs({false,true,42,{},string.char(255),"","../outside","output/a\0b","output/*.png",string.rep("a",4097)}) do
                    assert(not pcall(operation,path))
                end
            end
            assert(sai.fs.remove_file("output/a")==false)
            return "checked"
        end
        sai.register_tool({name="run",description="Removal inputs",access="writes",parameters={type="object"},execute=run})
    "#;
    let plugin = runtime(source, host.clone(), |limits| limits.system_calls = 1);
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "checked"
    );
    assert_eq!(host.calls.lock().unwrap().len(), 1);
}

/// 【删除绑定测试】【共用预算】成功、缺失和宿主错误都消耗同一系统调用额度
/// @returns 无；任一接口用满额度后，另一接口不能继续调用宿主
#[tokio::test]
async fn removal_results_and_errors_share_the_system_call_budget() {
    for (result, fail) in [(true, false), (false, false), (false, true)] {
        let host = Arc::new(Host::default());
        host.result.store(result, Ordering::SeqCst);
        host.fail.store(fail, Ordering::SeqCst);
        let plugin = runtime(
            r#"
            --- 【删除绑定测试】【额度共用】分别进入两个接口，再检查额度错误
            --- @return string 检查结果
            local function run()
                pcall(sai.fs.remove_file,"output/a")
                pcall(sai.fs.trash_file,"output/a")
                local ok,message=pcall(sai.fs.remove_file,"output/a")
                assert(not ok and tostring(message):find("system call budget exceeded",1,true),tostring(message))
                return "limited"
            end
            sai.register_tool({name="run",description="Removal budget",access="writes",parameters={type="object"},execute=run})
        "#,
            host.clone(),
            |limits| limits.system_calls = 2,
        );
        assert_eq!(
            plugin
                .call_tool("run", json!({}), context(true))
                .await
                .unwrap(),
            "limited"
        );
        assert_eq!(host.calls.lock().unwrap().len(), 2);
    }
}
