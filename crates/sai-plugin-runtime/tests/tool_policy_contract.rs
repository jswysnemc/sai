#[path = "reply_policy/support.rs"]
mod support;

use sai_plugin_runtime::{PluginRuntime, ToolPolicyInput};
use serde_json::{json, Value};

/// 【工具策略测试】【可信输入】构造与已完成工具对应的事实，工具目录采用插件本地名称
/// @param arguments 原工具参数
/// @returns 有界工具事实
fn input(arguments: Value) -> ToolPolicyInput {
    ToolPolicyInput {
        name: "native_tool".into(),
        local_name: None,
        arguments,
        ok: true,
        tools: vec!["todo".into()],
    }
}

/// 【工具策略测试】【函数装配】给单个工具后函数补齐兼容的回复回调
/// @param body 函数正文
/// @returns 具有独立回复策略授权的实例
fn runtime(body: &str) -> PluginRuntime {
    let source = format!("sai.register_reply_policy({{prepare=function()end,complete=function()end,after_tool=function(input,state,ctx) {body} end}})");
    support::load(&source, true, true, |_| {}).unwrap()
}

/// 【工具策略测试】【注册扩展】回复策略可以提供独立工具后回调，旧双回调仍兼容
/// @returns 无；未提供回调时继续沿用既有回复策略契约
#[test]
fn tool_policy_registers_optional_after_tool_callback() {
    let source = r#"
    sai.register_reply_policy({
        prepare=function() return nil end,
        complete=function() return nil end,
        after_tool=function(input, state, ctx) return {state={count=1}, reminder="check progress"} end,
    })
    "#;
    let loaded = support::load(source, true, true, |_| {});
    assert!(
        loaded.is_ok(),
        "after_tool must register: {:?}",
        loaded.err()
    );
}

/// 【工具策略测试】【状态与权限】循环状态显式传递，调用者写入许可不能改变观察回调的只读身份
/// @returns 无；旧回复策略没有可选工具后回调
#[tokio::test]
async fn tool_policy_preserves_state_and_forces_read_only() {
    let plugin = runtime("assert(ctx.allow_writes == false); local count=type(state)=='table' and state.count or 0; return {state={count=count+1},reminder=input.name}");
    assert!(plugin.has_tool_policy());
    let first = plugin
        .after_tool(input(json!({})), Value::Null, support::context(true))
        .await
        .unwrap();
    let second = plugin
        .after_tool(input(json!({})), first.state, support::context(true))
        .await
        .unwrap();
    assert_eq!(second.state, json!({"count":2}));
    assert_eq!(second.reminder.as_deref(), Some("native_tool"));
    assert!(!support::load(support::SOURCE, true, true, |_| {})
        .unwrap()
        .has_tool_policy());
}

/// 【工具策略测试】【独立授权】声明和用户授权缺一不可，不隐式沿用普通工具权限
/// @returns 无；失败之后不会执行策略函数
#[tokio::test]
async fn tool_policy_requires_declared_and_granted_permission() {
    let source = "sai.register_reply_policy({prepare=function()end,complete=function()end,after_tool=function() error('must not run') end})";
    for (declared, granted) in [(false, false), (false, true), (true, false)] {
        let plugin = support::load(source, declared, granted, |_| {}).unwrap();
        let error = plugin
            .after_tool(input(json!({})), Value::Null, support::context(true))
            .await
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("not allowed"));
    }
}

/// 【工具策略测试】【严格结果】拒绝错误形状、超限状态、控制字符和核心资源标记
/// @returns 无；每次失败都不会产生可以消费的部分结果
#[tokio::test]
async fn tool_policy_rejects_invalid_results() {
    for expression in [
        "{unknown=true}",
        "{state=sai.json.array()}",
        "{state=7}",
        "{state={text=string.rep('x',16385)}}",
        "{reminder=7}",
        "{reminder=string.rep('x',16385)}",
        "{reminder=string.char(0)}",
        "{reminder='<context-resource name=\"core\">'}",
        "{reminder='</context-resource>'}",
    ] {
        let plugin = runtime(&format!("return {expression}"));
        assert!(
            plugin
                .after_tool(input(json!({})), Value::Null, support::context(true))
                .await
                .is_err(),
            "{expression}"
        );
    }
}

/// 【工具策略测试】【输入边界】进入 Lua 前限制调用事实、原循环状态和必需归属
/// @returns 无；合法调用在错误后继续成功
#[tokio::test]
async fn tool_policy_rejects_oversized_inputs_and_missing_ownership() {
    let plugin = runtime("return {state={valid=true}}");
    assert!(plugin
        .after_tool(
            input(json!({"text":"x".repeat(65536)})),
            Value::Null,
            support::context(true)
        )
        .await
        .is_err());
    for state in [json!([]), json!(false), json!({"text":"x".repeat(16385)})] {
        assert!(plugin
            .after_tool(input(json!({})), state, support::context(true))
            .await
            .is_err());
    }
    let mut context = support::context(true);
    context.operation_id.clear();
    assert!(plugin
        .after_tool(input(json!({})), Value::Null, context)
        .await
        .is_err());
    assert_eq!(
        plugin
            .after_tool(input(json!({})), Value::Null, support::context(true))
            .await
            .unwrap()
            .state,
        json!({"valid":true})
    );
}

/// 【插件时间测试】【时刻一致】UTC 文本和毫秒值来自同一次时钟读取
/// @returns 无；保留毫秒 ID 和原 RFC3339 字段需要的精度
#[tokio::test]
async fn tool_policy_utc_clock_snapshot_uses_one_instant() {
    let plugin = runtime("return {state=sai.time.utc_now()}");
    let state = plugin
        .after_tool(input(json!({})), Value::Null, support::context(true))
        .await
        .unwrap()
        .state;
    let instant = chrono::DateTime::parse_from_rfc3339(state["rfc3339"].as_str().unwrap()).unwrap();
    assert_eq!(Some(instant.timestamp_millis()), state["unix_ms"].as_i64());
    assert_eq!(instant.offset().local_minus_utc(), 0);
}
