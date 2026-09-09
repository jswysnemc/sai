mod common;

use common::services::{context, runtime, RecordingServices};
use sai_plugin_runtime::host::ModelResponse;
use sai_plugin_runtime::{Capabilities, EventContext, EventKind};
use serde_json::{json, Value};
use std::sync::Arc;

const READ_AND_MODEL: &str = r#"
sai.register_tool({name="run",description="Service exercise",parameters={type="object"},execute=function(args,ctx)
    ctx.allow_writes = true
    local names = sai.tools.list()
    local denied_write, write_error = pcall(sai.tools.call, "write", {})
    local denied_extra = pcall(sai.tools.call, "undeclared", {})
    local output = sai.tools.call("read", '{"value":42}')
    local model = sai.model.complete({messages={{role="user",content="question"}},tools={"read"}})
    return {names=names,write_allowed=denied_write,write_error=tostring(write_error),
        extra_allowed=denied_extra,output=output,model=model}
end})
"#;

/// 【插件测试】【授权交集】声明、授权和只读回调共同限制目录、工具与模型请求。
#[tokio::test]
async fn service_capabilities_are_intersected_and_context_cannot_grant_writes() {
    let declared: Capabilities =
        serde_json::from_value(json!({"model":true,"tools":["read","write"]})).unwrap();
    let granted: Capabilities =
        serde_json::from_value(json!({"model":true,"tools":["read","write","undeclared"]}))
            .unwrap();
    let plugin = runtime(READ_AND_MODEL, declared, granted, |_| {});
    let services = Arc::new(RecordingServices::default());
    let text = plugin
        .call_tool("run", json!({}), context(services.clone(), true))
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(result["names"].as_array().unwrap().len(), 1);
    assert_eq!(result["names"][0]["name"], "read");
    assert_eq!(result["write_allowed"], false);
    assert_eq!(result["extra_allowed"], false);
    assert_eq!(result["output"], "tool result");
    assert_eq!(
        services.calls.lock().unwrap().as_slice(),
        &[("read".into(), "{\"value\":42}".into())]
    );
    assert_eq!(services.models.lock().unwrap()[0].tools, ["read"]);
}

/// 【插件测试】【模型撤销】缺少模型授权时不接触宿主模型服务。
#[tokio::test]
async fn revoked_model_capability_never_reaches_host() {
    let declared: Capabilities = serde_json::from_value(json!({"model":true})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Denied model",parameters={type="object"},execute=function()
            return sai.model.complete({messages={{role="user",content="question"}}})
        end})
    "#,
        declared,
        Capabilities::default(),
        |_| {},
    );
    let services = Arc::new(RecordingServices::default());
    let error = plugin
        .call_tool("run", json!({}), context(services.clone(), true))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("model capability is not allowed"));
    assert!(services.models.lock().unwrap().is_empty());
}

/// 【插件测试】【工具写入】写入工具还必须取得当前回调的写入许可。
#[tokio::test]
async fn writing_tool_requires_host_approval_and_declared_access() {
    let grants: Capabilities = serde_json::from_value(json!({"tools":["write"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Write",access="writes",parameters={type="object"},execute=function()
            return sai.tools.call("write", {value=1})
        end})
    "#,
        grants.clone(),
        grants,
        |_| {},
    );
    let services = Arc::new(RecordingServices::default());
    assert!(plugin
        .call_tool("run", json!({}), context(services.clone(), false))
        .await
        .is_err());
    assert!(services.calls.lock().unwrap().is_empty());
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(services.clone(), true))
            .await
            .unwrap(),
        "tool result"
    );
    assert_eq!(services.calls.lock().unwrap().len(), 1);
}

/// 【插件测试】【模型消息】非法消息、伪造配置和未授权工具在请求模型前被拒绝。
#[tokio::test]
async fn invalid_requests_do_not_reach_model() {
    let grants: Capabilities =
        serde_json::from_value(json!({"model":true,"tools":["read"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Request validation",parameters={type="object"},execute=function()
            local requests = {
                {messages={}},
                {messages={{role="assistant",content="unfinished"}}},
                {messages={{role="user",content="ok"}},api_key="forged"},
                {messages={{role="user",content="ok"}},tools={"undeclared"}},
                {messages={{role="user",content=string.rep("x",2048)}}},
            }
            for _, request in ipairs(requests) do
                local ok = pcall(sai.model.complete, request)
                assert(not ok)
            end
            return "rejected"
        end})
    "#,
        grants.clone(),
        grants,
        |limits| limits.output_bytes = 1024,
    );
    let services = Arc::new(RecordingServices::default());
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(services.clone(), false))
            .await
            .unwrap(),
        "rejected"
    );
    assert!(services.models.lock().unwrap().is_empty());
}

/// 【插件测试】【次数预算】每次回调独立计数，Lua 修改限制副本不能扩大实际额度。
#[tokio::test]
async fn model_and_tool_budgets_reset_without_being_mutable_from_lua() {
    let grants: Capabilities =
        serde_json::from_value(json!({"model":true,"tools":["read"]})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Budgets",parameters={type="object"},execute=function()
            sai.limits.model_requests = 1000
            sai.limits.tool_calls = 1000
            sai.model.complete({messages={{role="user",content="one"}}})
            local model_ok, model_error = pcall(sai.model.complete,{messages={{role="user",content="two"}}})
            sai.tools.call("read",{})
            local tool_ok, tool_error = pcall(sai.tools.call,"read",{})
            return {model_ok=model_ok,model_error=tostring(model_error),tool_ok=tool_ok,tool_error=tostring(tool_error)}
        end})
    "#,
        grants.clone(),
        grants,
        |limits| {
            limits.model_requests = 1;
            limits.tool_calls = 1;
        },
    );
    let services = Arc::new(RecordingServices::default());
    for _ in 0..2 {
        let text = plugin
            .call_tool("run", json!({}), context(services.clone(), false))
            .await
            .unwrap();
        let result: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(result["model_ok"], false);
        assert_eq!(result["tool_ok"], false);
        assert!(result["model_error"]
            .as_str()
            .unwrap()
            .contains("budget exceeded"));
        assert!(result["tool_error"]
            .as_str()
            .unwrap()
            .contains("budget exceeded"));
    }
    assert_eq!(services.models.lock().unwrap().len(), 2);
    assert_eq!(services.calls.lock().unwrap().len(), 2);
}

/// 【插件测试】【结果限制】超大宿主响应不会作为插件输出继续传播。
#[tokio::test]
async fn oversized_model_response_is_catchable() {
    let grants: Capabilities = serde_json::from_value(json!({"model":true})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Response size",parameters={type="object"},execute=function()
            local ok, err = pcall(sai.model.complete,{messages={{role="user",content="question"}}})
            assert(not ok)
            return tostring(err)
        end})
    "#,
        grants.clone(),
        grants,
        |limits| limits.output_bytes = 1024,
    );
    let services = Arc::new(RecordingServices::default());
    services.responses.lock().unwrap().push_back(ModelResponse {
        content: "x".repeat(2048),
        ..Default::default()
    });
    let result = plugin
        .call_tool("run", json!({}), context(services, false))
        .await
        .unwrap();
    assert!(result.contains("model response exceeds size limit"));
}

/// 【插件测试】【事件隔离】事件不能复用上一个工具回调的模型授权上下文。
#[tokio::test]
async fn events_do_not_retain_previous_invocation_services() {
    let grants: Capabilities = serde_json::from_value(json!({"model":true})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name="run",description="Model call",parameters={type="object"},execute=function()
            return sai.model.complete({messages={{role="user",content="question"}}}).content
        end})
        sai.on("agent_start",function()
            local ok, err = pcall(sai.model.complete,{messages={{role="user",content="event"}}})
            assert(not ok)
            return tostring(err)
        end)
    "#,
        grants.clone(),
        grants,
        |_| {},
    );
    let services = Arc::new(RecordingServices::default());
    let weak = Arc::downgrade(&services);
    plugin
        .call_tool("run", json!({}), context(services.clone(), false))
        .await
        .unwrap();
    drop(services);
    assert!(
        weak.upgrade().is_none(),
        "completed invocation retained its host services"
    );
    let output = plugin
        .emit(EventKind::AgentStart, EventContext::default())
        .await
        .unwrap();
    assert!(output[0]
        .as_str()
        .unwrap()
        .contains("services are unavailable"));
}
