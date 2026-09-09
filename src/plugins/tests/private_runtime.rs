use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    Capabilities, EventContext, EventKind, InvocationContext, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::sync::Arc;

/// 【私有运行时测试】【脚本】绑定临时路径与显式授权，不访问真实应用数据。
/// @param paths 临时应用路径；source 为脚本；grants 为显式授权
/// @returns 正式运行时
fn runtime(paths: &SaiPaths, source: &str, grants: Capabilities) -> PluginRuntime {
    let mut descriptor = super::support::descriptor("private-test", source);
    descriptor.package.manifest.capabilities = super::private_storage::capabilities();
    let sources = descriptor.package.sources().clone();
    let package = PluginPackage::new(descriptor.package.manifest, sources).unwrap();
    PluginRuntime::load(
        package,
        json!({}),
        grants,
        Arc::new(PrivatePluginHost::new(paths, "private-test")),
    )
    .unwrap()
}

/// 【私有运行时测试】【可信归属】修改 Lua 上下文不改变状态归属，事件写入仍受禁止。
#[tokio::test]
async fn private_runtime_binds_trusted_sessions_and_denies_event_mutations() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let plugin = runtime(
        &paths,
        r#"
        sai.register_tool({name="state",description="state",parameters={type="object"},execute=function(args, ctx)
            ctx.session_id = "forged"
            ctx.storage_session_id = "forged"
            if args.write then sai.storage.set("key", args.write) end
            return sai.storage.get("key")
        end})
        sai.on("agent_start", function()
            local state_ok = pcall(sai.storage.set, "key", 99)
            local workspace_ok = pcall(sai.workspace.open, "key")
            return {state_ok=state_ok, workspace_ok=workspace_ok}
        end)
    "#,
        super::private_storage::capabilities(),
    );
    let ctx = InvocationContext {
        session_id: "real".into(),
        ..Default::default()
    };
    assert_eq!(
        plugin
            .call_tool("state", json!({"write":42}), ctx.clone())
            .await
            .unwrap(),
        "42"
    );
    let event = plugin
        .emit(
            EventKind::AgentStart,
            EventContext {
                session_id: "real".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(event, vec![json!({"state_ok":false,"workspace_ok":false})]);
    assert_eq!(
        plugin.call_tool("state", json!({}), ctx).await.unwrap(),
        "42"
    );
    assert_eq!(
        plugin
            .call_tool(
                "state",
                json!({}),
                InvocationContext {
                    session_id: "forged".into(),
                    ..Default::default()
                }
            )
            .await
            .unwrap(),
        ""
    );
}

/// 【私有运行时测试】【句柄期限】工作目录可在同一回调使用，回调结束后旧句柄失效并释放锁。
#[tokio::test]
async fn private_runtime_workspace_handles_expire_and_release_locks() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let plugin = runtime(
        &paths,
        r#"
        local previous
        sai.register_tool({name="open",description="open",parameters={type="object"},execute=function()
            assert(coroutine == nil and io == nil and os == nil)
            if previous then assert(not pcall(function() return previous:path() end)) end
            previous = sai.workspace.open("same")
            assert(not pcall(sai.workspace.open, "same"))
            assert(not pcall(function() return previous:stat("../outside") end))
            assert(not pcall(function() return previous:stat("C:/outside") end))
            return previous:read_dir(".").entries
        end})
    "#,
        super::private_storage::capabilities(),
    );
    for _ in 0..2 {
        assert_eq!(
            plugin
                .call_tool("open", json!({}), InvocationContext::default())
                .await
                .unwrap(),
            "[]"
        );
    }
}

/// 【私有运行时测试】【缺省拒绝】未授予私有能力时，初始化及执行阶段都不能创建状态或目录。
#[tokio::test]
async fn private_runtime_does_not_inherit_undeclared_private_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let plugin = runtime(
        &paths,
        r#"
        assert(not pcall(sai.workspace.open, "key"))
        assert(not pcall(sai.storage.set, "key", 1))
        sai.register_tool({name="denied",description="denied",parameters={type="object"},execute=function()
            assert(not pcall(sai.workspace.open, "key"))
            assert(not pcall(sai.storage.get, "key"))
            return true
        end})
    "#,
        Capabilities::default(),
    );
    assert_eq!(
        plugin
            .call_tool("denied", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "true"
    );
    assert!(!paths.state_dir.join("plugin-state").exists());
    assert!(!paths.cache_dir.join("plugin-workspaces").exists());
}
