use super::support::{descriptor, FixtureHost};
use crate::plugins::registry::register_descriptor;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::Arc;

const COUNTER: &str = r#"
local calls = 0
sai.register_tool({name='count', description='Original plugin description', parameters={type='object',additionalProperties=false}, execute=function(args, ctx)
    calls = calls + 1
    return {calls=calls, session_id=ctx.session_id}
end})
sai.register_command({name='stats',description='Read counter',execute=function() return calls end})
"#;

/// 【插件测试】【状态所有权】注册表过滤保持同一实例，新 Agent 和恢复的新会话拥有独立 VM。
#[tokio::test]
async fn registry_clones_share_state_and_new_agents_do_not() {
    let mut parent = ToolRegistry::new();
    register_descriptor(
        &mut parent,
        descriptor("counter", COUNTER),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    parent.start_plugin_session("parent").unwrap();
    let read = |text: String| serde_json::from_str::<Value>(&text).unwrap();
    let name = "lua__counter__count";
    assert_eq!(
        read(parent.call(name, "{}").await.unwrap()),
        json!({"calls":1,"session_id":"parent"})
    );
    let filtered = parent.clone_filtered(&[name]);
    assert_eq!(read(filtered.call(name, "{}").await.unwrap())["calls"], 2);
    let mut child = parent.clone_excluding(&[]);
    child.start_plugin_session("child").unwrap();
    assert_eq!(
        read(child.call(name, "{}").await.unwrap()),
        json!({"calls":1,"session_id":"child"})
    );
    assert_eq!(read(parent.call(name, "{}").await.unwrap())["calls"], 3);
    let stats = parent.plugin_command("counter", "stats").unwrap();
    let stats_name = stats.name.clone();
    parent.register(stats);
    assert_eq!(
        parent
            .call(&stats_name, r#"{"arguments":""}"#)
            .await
            .unwrap(),
        "3"
    );
    assert_eq!(
        parent.definition(name).unwrap().function.description,
        "Original plugin description"
    );
}

/// 【插件测试】【加载事务】一个名字冲突会拒绝完整包，不留下先注册的工具或命令。
#[test]
fn collisions_and_long_names_reject_the_entire_plugin() {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "lua__conflict__z",
        "Core tool",
        json!({"type":"object"}),
        |_| async { Ok("core".into()) },
    ));
    let source = r#"
        for _, name in ipairs({'a','z'}) do
            sai.register_tool({name=name,description='Test tool',parameters={type='object'},execute=function() return 'plugin' end})
        end
        sai.register_command({name='command',description='Test command',execute=function() end})
    "#;
    assert!(register_descriptor(
        &mut registry,
        descriptor("conflict", source),
        Arc::new(FixtureHost::default()),
        false
    )
    .is_err());
    assert!(!registry.contains("lua__conflict__a"));
    assert!(registry.plugin_commands().is_empty());
    let source = format!("sai.register_tool({{name='{}',description='Too long',parameters={{type='object'}},execute=function() end}})", "a".repeat(48));
    assert!(register_descriptor(
        &mut registry,
        descriptor(&"a".repeat(32), &source),
        Arc::new(FixtureHost::default()),
        false
    )
    .is_err());
}

/// 【插件测试】【执行检查】插件拒绝和无效授权返回值都不能绕过宿主执行入口。
#[tokio::test]
async fn tool_checks_can_only_deny_and_observe_real_results() {
    let mut registry = ToolRegistry::new();
    let source = r#"
        local failed = 0
        sai.on('tool_call', function(event)
            if event.name == 'blocked' then return {deny='project policy'} end
            if event.name == 'invalid' then return {allow=true} end
        end)
        sai.on('tool_result', function(event) if not event.ok then failed=failed+1 end end)
        sai.register_tool({name='failures',description='Observed failures',parameters={type='object'},execute=function() return failed end})
    "#;
    register_descriptor(
        &mut registry,
        descriptor("policy", source),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    for name in ["blocked", "invalid"] {
        registry.register(ToolSpec::new(
            name,
            "Must not execute",
            json!({"type":"object"}),
            |_| async { panic!("denied tool executed") },
        ));
        assert!(registry.call(name, "{}").await.is_err());
    }
    assert_eq!(
        registry.call("lua__policy__failures", "{}").await.unwrap(),
        "2"
    );
}

/// 【插件测试】【权限交集】写入工具经过 Sai 计划模式和 Lua 网络授权双重检查。
#[tokio::test]
async fn writing_tools_obey_host_permissions_and_readonly_catalogs() {
    let source = r#"sai.register_tool({name='write',description='Writing request',access='writes',parameters={type='object',additionalProperties=false},execute=function()
        return sai.http.request({url='https://example.test/',method='POST',body='payload'}).text
    end})"#;
    let mut plugin = descriptor("network", source);
    plugin.setting.grants = Some(plugin.package.manifest.capabilities.clone());
    let host = Arc::new(FixtureHost::new(&[(200, "written")]));
    let mut readonly = ToolRegistry::new();
    register_descriptor(&mut readonly, plugin.clone(), host.clone(), true).unwrap();
    assert!(!readonly.contains("lua__network__write"));
    let mut registry = ToolRegistry::new();
    register_descriptor(&mut registry, plugin, host.clone(), false).unwrap();
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        crate::permission::PermissionProfileMode::Plan,
        std::env::current_dir().unwrap(),
        None,
    ));
    assert!(registry.call("lua__network__write", "{}").await.is_err());
    assert!(host.requests.lock().unwrap().is_empty());
    registry.set_permission_mode(crate::permission::PermissionProfileMode::Yolo);
    assert_eq!(
        registry.call("lua__network__write", "{}").await.unwrap(),
        "written"
    );
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【插件测试】【工具表替换】未变更源码沿用状态，已变更源码采用新 VM。
#[tokio::test]
async fn replacing_tool_registry_preserves_only_unchanged_plugins() {
    let mut first = ToolRegistry::new();
    register_descriptor(
        &mut first,
        descriptor("counter", COUNTER),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    first.start_plugin_session("session").unwrap();
    first.call("lua__counter__count", "{}").await.unwrap();
    let mut next = ToolRegistry::new();
    register_descriptor(
        &mut next,
        descriptor("counter", COUNTER),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    next.continue_plugin_session(&first);
    let result: Value =
        serde_json::from_str(&next.call("lua__counter__count", "{}").await.unwrap()).unwrap();
    assert_eq!(result, json!({"calls":2,"session_id":"session"}));
    let mut changed = ToolRegistry::new();
    register_descriptor(
        &mut changed,
        descriptor("counter", &format!("{COUNTER}\n-- new version")),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    changed.continue_plugin_session(&next);
    let result: Value =
        serde_json::from_str(&changed.call("lua__counter__count", "{}").await.unwrap()).unwrap();
    assert_eq!(result, json!({"calls":1,"session_id":"session"}));
    let mut disabled = ToolRegistry::new();
    disabled.continue_plugin_session(&changed);
    assert!(disabled.plugin_command("counter", "stats").is_err());
    assert!(disabled.call("lua__counter__count", "{}").await.is_err());
}
