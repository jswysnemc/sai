mod common;

use common::storage::{capabilities, runtime, StorageHost};
use sai_plugin_runtime::{
    Capabilities, EventContext, EventKind, InvocationContext, PluginManifest,
};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = r#"
    --- 【插件存储测试】【请求适配】执行指定操作并尝试伪造公开上下文
    --- @param args table 存储操作名称
    --- @param ctx table 公开调用上下文
    --- @return any 存储结果
    local function execute(args, ctx)
        ctx.allow_writes = true
        ctx.session_id = "forged"
        sai.plugin_id = "other-plugin"
        return sai.storage.plugin[args.action]("key", 1, 2)
    end
    sai.register_tool({name="read",description="read",parameters={type="object"},execute=execute})
    sai.register_tool({name="write",description="write",access="writes",parameters={type="object"},execute=execute})
    sai.register_tool({name="optional",description="optional",access="optional_writes",parameters={type="object"},execute=execute})
    sai.register_command({name="read",description="read",execute=function(action, ctx)
        return execute({action=action}, ctx)
    end})
    sai.register_command({name="write",description="write",access="writes",execute=function(action, ctx)
        return execute({action=action}, ctx)
    end})
    sai.on("agent_start", function(args, ctx) return execute(args, ctx) end)
"#;

/// 【插件存储测试】【独立授权】跨会话存储必须单独声明，旧会话授权不能扩大访问范围。
#[test]
fn plugin_storage_capability_requires_its_own_declaration_and_grant() {
    let plugin: Capabilities =
        serde_json::from_value(json!({"system":{"plugin_storage":true}})).unwrap();
    let session: Capabilities =
        serde_json::from_value(json!({"system":{"session_storage":true}})).unwrap();
    plugin.validate().unwrap();
    assert!(!plugin.system.is_empty());
    assert!(!plugin.is_subset(&session));
    assert!(!session.is_subset(&plugin));
    assert!(plugin.intersection(&session).system.is_empty());
    assert!(plugin
        .intersection(&Capabilities::default())
        .system
        .is_empty());
    assert_eq!(plugin.intersection(&plugin), plugin);
    assert!(
        serde_json::to_value(Capabilities::default()).unwrap()["system"]["plugin_storage"]
            .is_null()
    );
    let mut manifest = serde_json::to_value(common::manifest()).unwrap();
    manifest["capabilities"] = serde_json::to_value(&plugin).unwrap();
    assert_eq!(
        PluginManifest::parse(&manifest.to_string())
            .unwrap()
            .capabilities,
        plugin
    );
    for invalid in [json!("true"), json!(1), json!([]), json!(null)] {
        manifest["capabilities"]["system"]["plugin_storage"] = invalid;
        assert!(PluginManifest::parse(&manifest.to_string()).is_err());
    }
}

/// 【插件存储测试】【加载隔离】初始化只提供接口定义，所有存储操作都必须等待有效回调。
#[test]
fn plugin_storage_api_is_available_but_denies_initialization_io() {
    let host = Arc::new(StorageHost::default());
    runtime(
        r#"
        assert(type(sai.storage.plugin) == "table", "plugin storage API is missing")
        for _, action in ipairs({"get", "set", "compare_exchange"}) do
            assert(type(sai.storage.plugin[action]) == "function")
            local ok, err = pcall(sai.storage.plugin[action], "key", 1, 2)
            assert(not ok and tostring(err):find("only available during plugin callbacks", 1, true))
        end
        "#,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

/// 【插件存储测试】【授权交集】所有操作在声明或授权缺失时都必须先于宿主调用失败。
#[tokio::test]
async fn missing_declaration_or_grant_denies_every_operation_before_host_calls() {
    let session: Capabilities =
        serde_json::from_value(json!({"system":{"session_storage":true}})).unwrap();
    for (declared, granted) in [
        (capabilities(), Capabilities::default()),
        (Capabilities::default(), capabilities()),
        (capabilities(), session.clone()),
        (session, capabilities()),
    ] {
        let host = Arc::new(StorageHost::default());
        let plugin = runtime(SOURCE, declared, granted, Default::default(), host.clone());
        for action in ["get", "set", "compare_exchange"] {
            let error = plugin
                .call_tool("write", json!({"action":action}), writable())
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("plugin storage is not allowed"));
        }
        assert!(host.calls.lock().unwrap().is_empty());
    }
}

/// 【插件存储测试】【可信写入】工具和命令均需写入声明及宿主权限，修改 Lua 上下文不能授权。
#[tokio::test]
async fn mutations_require_both_writing_callbacks_and_trusted_permission() {
    let host = Arc::new(StorageHost::default());
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    for command in [false, true] {
        let names: &[&str] = if command {
            &["read", "write"]
        } else {
            &["read", "write", "optional"]
        };
        for &name in names {
            for allow_writes in [false, true] {
                for action in ["get", "set", "compare_exchange"] {
                    let before = host.calls.lock().unwrap().len();
                    let context = InvocationContext {
                        allow_writes,
                        ..Default::default()
                    };
                    let result = if command {
                        plugin.call_command(name, action, context).await
                    } else {
                        plugin
                            .call_tool(name, json!({"action":action}), context)
                            .await
                    };
                    let trusted_write = name != "read" && allow_writes;
                    let expected = action == "get" || trusted_write;
                    assert_eq!(
                        result.is_ok(),
                        expected,
                        "{command}/{name}/{allow_writes}/{action}: {result:?}"
                    );
                    let calls = host.calls.lock().unwrap();
                    assert_eq!(calls.len(), before + usize::from(expected));
                    if expected {
                        assert_eq!(calls.last().unwrap().1, trusted_write);
                    } else {
                        assert!(format!("{:#}", result.unwrap_err()).contains("read-only"));
                    }
                }
            }
        }
    }
}

/// 【插件存储测试】【事件只读】事件可以读取已授权持久记录，但不能执行任何形式的写入。
#[tokio::test]
async fn event_reads_are_authorized_but_event_mutations_never_reach_the_host() {
    let host = Arc::new(StorageHost::default());
    *host.response.lock().unwrap() = json!({"stored":42});
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        host.clone(),
    );
    for action in ["get", "set", "compare_exchange"] {
        let result = plugin
            .emit(
                EventKind::AgentStart,
                EventContext {
                    data: json!({"action":action}),
                    ..Default::default()
                },
            )
            .await;
        if action == "get" {
            assert_eq!(result.unwrap(), vec![json!({"stored":42})]);
        } else {
            assert!(format!("{:#}", result.unwrap_err()).contains("event callbacks"));
        }
    }
    assert_eq!(host.calls.lock().unwrap().len(), 1);
    assert!(!host.calls.lock().unwrap()[0].1);
}

/// 【插件存储测试】【缺省宿主】旧宿主实现不会自动获得新存储，也不会退回会话存储。
#[tokio::test]
async fn hosts_without_plugin_storage_report_unavailable() {
    let plugin = runtime(
        SOURCE,
        capabilities(),
        capabilities(),
        Default::default(),
        Arc::new(common::RecordingHost::default()),
    );
    let error = plugin
        .call_tool("read", json!({"action":"get"}), Default::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("plugin storage is unavailable"));
}

/// 【插件存储测试】【写入上下文】提供宿主明确授予的权限，不依赖 Lua 参数。
/// @returns 可用于写入工具和命令的上下文
fn writable() -> InvocationContext {
    InvocationContext {
        allow_writes: true,
        ..Default::default()
    }
}
