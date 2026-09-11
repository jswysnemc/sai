use super::support::{descriptor, FixtureHost};
use crate::{
    plugins::{operation, registry::register_descriptor},
    tools::{PluginToolPolicyStates, ToolRegistry},
};
use serde_json::{json, Value};
use std::sync::Arc;

const SOURCE: &str = r#"
sai.register_tool({name="probe",description="Probe",parameters={type="object"},execute=function()return "ok"end})
sai.register_reply_policy({prepare=function()end,complete=function()end,
    after_tool=function(input,state,ctx)
        if input.arguments.fail then error("callback failure") end
        local count=type(state)=="table" and state.count or 0
        local next={count=count+1}
        return {state=next,reminder=sai.json.encode({count=next.count,local_name=input.local_name,tools=input.tools,ok=input.ok})}
    end})
"#;

/// 【工具策略分发测试】【注册】建立单个策略实例与真实公开名称映射
/// @param granted 是否授权策略；source 为注册源码
/// @returns 启动会话后的工具表和公开工具名称
fn registry(granted: bool, source: &str) -> (ToolRegistry, String) {
    let mut plugin = descriptor("tool-policy-test", source);
    plugin.package.manifest.capabilities.reply_policy = true;
    let mut grants = plugin.package.manifest.capabilities.clone();
    grants.reply_policy = granted;
    plugin.setting.grants = Some(grants);
    let name = plugin.tool_name("probe").unwrap();
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        plugin,
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    registry.start_plugin_session("policy-session").unwrap();
    (registry, name)
}

/// 【工具策略分发测试】【状态读取】只解析已校验的回调提醒，检查宿主实际传入的事实
/// @param registry 当前表；states 为循环状态；name 为刚完成工具
/// @returns 回调的事实快照
async fn observe(
    registry: &ToolRegistry,
    states: &mut PluginToolPolicyStates,
    name: &str,
) -> Value {
    let output = registry.after_plugin_tool(states, name, "{}", true).await;
    assert_eq!(output.len(), 1);
    serde_json::from_str(&output[0]).unwrap()
}

/// 【工具策略分发测试】【归属隔离】会话、操作、存储归属、工作目录和实例变化均清空旧状态
/// @returns 无；只有同一操作的同一实例可以延续状态
#[tokio::test]
async fn tool_policy_dispatch_binds_state_to_operation_session_workdir_and_instance() {
    let (mut registry, name) = registry(true, SOURCE);
    let mut states = PluginToolPolicyStates::default();
    operation::scope("first", async {
        let first = observe(&registry, &mut states, &name).await;
        assert_eq!(first["count"], 1);
        assert_eq!(first["local_name"], "probe");
        assert_eq!(first["tools"], json!(["probe"]));
        assert_eq!(observe(&registry, &mut states, &name).await["count"], 2);
        registry.inherit_plugin_storage_session("other-storage");
        assert_eq!(observe(&registry, &mut states, &name).await["count"], 1);
        let root = tempfile::tempdir().unwrap();
        crate::runtime_cwd::scope(root.path().to_path_buf(), async {
            assert_eq!(observe(&registry, &mut states, &name).await["count"], 1);
            assert_eq!(observe(&registry, &mut states, &name).await["count"], 2);
        })
        .await;
    })
    .await;
    operation::scope("second", async {
        assert_eq!(observe(&registry, &mut states, &name).await["count"], 1);
        registry.start_plugin_session("another-session").unwrap();
        assert_eq!(observe(&registry, &mut states, &name).await["count"], 1);
        registry.start_plugin_session("another-session").unwrap();
        assert_eq!(observe(&registry, &mut states, &name).await["count"], 1);
    })
    .await;
}

/// 【工具策略分发测试】【过滤和撤权】回调只接收允许目录，失败结果不能替换上次状态
/// @returns 无；未授权策略不会运行或继承先前状态
#[tokio::test]
async fn tool_policy_dispatch_filters_tools_and_discards_failed_results() {
    let (active, name) = registry(true, SOURCE);
    let (denied, _) = registry(false, SOURCE);
    let mut states = PluginToolPolicyStates::default();
    operation::scope("same", async {
        assert_eq!(observe(&active, &mut states, &name).await["count"], 1);
        assert!(active
            .after_plugin_tool(&mut states, &name, r#"{"fail":true}"#, false)
            .await
            .is_empty());
        assert_eq!(observe(&active, &mut states, &name).await["count"], 2);
        let filtered = active.clone_filtered(&[]);
        assert_eq!(
            observe(&filtered, &mut states, "native").await["tools"],
            json!([])
        );
        assert!(denied
            .after_plugin_tool(&mut states, &name, "{}", true)
            .await
            .is_empty());
        assert_eq!(observe(&active, &mut states, &name).await["count"], 1);
    })
    .await;
}

/// 【工具策略分发测试】【数量边界】策略不依赖工具注册，每次只执行稳定排序前八项
/// @returns 无；更多有效策略不扩大单轮输出数量
#[tokio::test]
async fn tool_policy_dispatch_bounds_the_number_of_callbacks() {
    let mut registry = ToolRegistry::new();
    for number in 0..10 {
        let id = format!("policy-{number:02}");
        let source = format!("sai.register_reply_policy({{prepare=function()end,complete=function()end,after_tool=function()return {{reminder='{id}'}}end}})");
        let mut plugin = descriptor(&id, &source);
        plugin.package.manifest.capabilities.reply_policy = true;
        plugin.setting.grants = Some(plugin.package.manifest.capabilities.clone());
        register_descriptor(
            &mut registry,
            plugin,
            Arc::new(FixtureHost::default()),
            false,
        )
        .unwrap();
    }
    registry.start_plugin_session("bounded").unwrap();
    let output = registry
        .after_plugin_tool(&mut PluginToolPolicyStates::default(), "native", "{}", true)
        .await;
    assert_eq!(
        output,
        (0..8)
            .map(|number| format!("policy-{number:02}"))
            .collect::<Vec<_>>()
    );
}
