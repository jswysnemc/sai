use super::support::{descriptor, FixtureHost};
use crate::plugins::registry::register_descriptor;
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【插件测试】【生命周期配对】真实分发器在成功和失败路径按顺序报告同一逻辑请求。
#[tokio::test]
async fn lifecycle_events_are_ordered_and_failures_preserve_the_original_error() {
    let source = r#"
        local events = sai.json.array()
        for _, name in ipairs({'agent_start','agent_end','turn_start','turn_end','message_start','message_end'}) do
            sai.on(name, function(event,ctx) events[#events+1] = {name=name,ok=event.ok,session_id=ctx.session_id} end)
        end
        sai.register_tool({name='events',description='Read observed events',parameters={type='object'},execute=function() return events end})
    "#;
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        descriptor("observer", source),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    registry.start_plugin_session("test-session").unwrap();
    let events = registry.plugin_events();
    let result: anyhow::Result<()> = events
        .agent_run(
            json!({"turn_id":"t1"}),
            events.model_round(json!({"round":1}), async {
                anyhow::bail!("original failure")
            }),
        )
        .await;
    assert_eq!(result.unwrap_err().to_string(), "original failure");
    let recorded: Value =
        serde_json::from_str(&registry.call("lua__observer__events", "{}").await.unwrap()).unwrap();
    assert_eq!(
        recorded,
        json!([
            {"name":"agent_start","session_id":"test-session"},
            {"name":"turn_start","session_id":"test-session"},
            {"name":"message_start","session_id":"test-session"},
            {"name":"message_end","session_id":"test-session","ok":false},
            {"name":"turn_end","session_id":"test-session","ok":false},
            {"name":"agent_end","session_id":"test-session","ok":false},
        ])
    );
    events.agent_run(json!({}), async { Ok(()) }).await.unwrap();
    let recorded: Value =
        serde_json::from_str(&registry.call("lua__observer__events", "{}").await.unwrap()).unwrap();
    assert_eq!(recorded[7]["ok"], true);
}

/// 【插件测试】【观察者隔离】坏监听器不会阻断其他插件或改写真实模型结果。
#[tokio::test]
async fn failing_observers_do_not_replace_the_operation_result() {
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        descriptor(
            "broken",
            "sai.on('agent_end', function() error('observer failed') end)",
        ),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    let output = registry
        .plugin_events()
        .agent_run(json!({}), async { Ok("original output") })
        .await
        .unwrap();
    assert_eq!(output, "original output");
}

/// 【插件测试】【Agent 入口】实际 Agent 构造和会话切换必须新建实例，直接命令使用当前会话状态。
#[tokio::test]
async fn agent_construction_and_session_switching_isolate_lua_state() {
    let source = r#"
        local n=0
        sai.register_command({name='next',description='Increment counter',execute=function(args,ctx)
            n=n+1
            return {n=n,session_id=ctx.session_id}
        end})
    "#;
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let config = crate::config::AppConfig::default();
    let mut registry = ToolRegistry::new();
    register_descriptor(
        &mut registry,
        descriptor("counter", source),
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    let create = || {
        let state = crate::state::StateStore::new(&paths).unwrap();
        state.init_files().unwrap();
        crate::agent::Agent::new(
            config.clone(),
            &paths,
            state,
            crate::llm::OpenAiCompatibleClient::from_config(&config, &paths).unwrap(),
            registry.clone(),
            crate::agent::AgentMode::Plan,
        )
        .unwrap()
    };
    let mut first = create();
    let second = create();
    let (first_tools, name) = first.plugin_command_registry("counter", "next").unwrap();
    let read = |text: String| serde_json::from_str::<Value>(&text).unwrap();
    assert_eq!(
        read(
            first_tools
                .call(&name, r#"{"arguments":""}"#)
                .await
                .unwrap()
        ),
        json!({"n":1,"session_id":first.session_id()})
    );
    let (same_tools, _) = first.plugin_command_registry("counter", "next").unwrap();
    assert_eq!(
        read(same_tools.call(&name, r#"{"arguments":""}"#).await.unwrap())["n"],
        2
    );
    let (second_tools, _) = second.plugin_command_registry("counter", "next").unwrap();
    assert_eq!(
        read(
            second_tools
                .call(&name, r#"{"arguments":""}"#)
                .await
                .unwrap()
        ),
        json!({"n":1,"session_id":second.session_id()})
    );
    let next_state = crate::state::StateStore::new(&paths).unwrap();
    next_state.init_files().unwrap();
    first.replace_state(next_state).unwrap();
    let (switched_tools, _) = first.plugin_command_registry("counter", "next").unwrap();
    assert_eq!(
        read(
            switched_tools
                .call(&name, r#"{"arguments":""}"#)
                .await
                .unwrap()
        ),
        json!({"n":1,"session_id":first.session_id()})
    );
}
