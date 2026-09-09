use super::services_support::{register_service, tool_call, ModelFixture, ModelReply};
use crate::agent::{Agent, AgentMode};
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::{
    ProgressMode, SubagentProgress, SubagentRunner, ToolProgress, ToolRegistry, ToolSpec,
};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const MODEL_PLUGIN: &str = r#"
local n=0
--- 【插件测试】【模型查询】调用当前宿主模型并保留本实例的调用次数
--- @return table 模型正文与累计次数
local function query()
    n=n+1
    local result=sai.model.complete({messages={{role='user',content='fixture request'}}})
    return {content=result.content, n=n}
end
sai.register_tool({name='run',description='Query current model',parameters={type='object'},execute=query})
sai.register_command({name='query',description='Query current model',execute=query})
"#;

/// 【插件测试】【模型工具表】建立带过期默认配置的工具表，供 Agent 覆盖为实际选定客户端。
/// @param fixture 本地模型服务；paths 为临时目录
/// @returns 含实际模型插件的注册表
fn registry(fixture: &ModelFixture, paths: &SaiPaths) -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.configure_plugin_model(&fixture.config("configured-default"), paths);
    register_service(&mut tools, "model", MODEL_PLUGIN, json!({"model":true}));
    tools
}

/// 【插件测试】【Agent 命令】经正式 Agent 命令入口请求当前模型。
/// @param agent 当前 Agent
/// @returns 插件 JSON 结果
async fn query_agent(agent: &Agent) -> Value {
    let (tools, name) = agent.plugin_command_registry("model", "query").unwrap();
    serde_json::from_str(&tools.call(&name, r#"{"arguments":""}"#).await.unwrap()).unwrap()
}

/// 【插件测试】【实际模型继承】新建、重载、切换模式和替换工具表都使用当前客户端，未变插件保持 Lua 状态。
#[tokio::test]
async fn agent_lifecycle_refreshes_model_without_resetting_unchanged_lua_state() {
    let fixture = ModelFixture::start(
        (1..=5)
            .map(|n| ModelReply::text(&format!("response-{n}")))
            .collect(),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let mut agent = Agent::new(
        fixture.config("configured-default"),
        &paths,
        state,
        fixture.client("initial-client", &paths),
        registry(&fixture, &paths),
        AgentMode::Plan,
    )
    .unwrap();
    assert_eq!(
        query_agent(&agent).await,
        json!({"content":"response-1", "n":1})
    );
    agent
        .reload(
            fixture.config("configured-after-reload"),
            fixture.client("selected-after-reload", &paths),
            registry(&fixture, &paths),
            AgentMode::Plan,
        )
        .unwrap();
    assert_eq!(
        query_agent(&agent).await,
        json!({"content":"response-2", "n":2})
    );
    agent
        .switch_mode(AgentMode::Yolo, registry(&fixture, &paths))
        .unwrap();
    assert_eq!(
        query_agent(&agent).await,
        json!({"content":"response-3", "n":3})
    );
    agent.replace_tools(registry(&fixture, &paths));
    assert_eq!(
        query_agent(&agent).await,
        json!({"content":"response-4", "n":4})
    );
    let (tools, name) = agent.plugin_command_registry("model", "query").unwrap();
    let filtered = tools.clone_filtered(&[&name]).clone_excluding(&["missing"]);
    let value: Value =
        serde_json::from_str(&filtered.call(&name, r#"{"arguments":""}"#).await.unwrap()).unwrap();
    assert_eq!(value, json!({"content":"response-5", "n":5}));
    let requests = fixture.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request["model"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "initial-client",
            "selected-after-reload",
            "selected-after-reload",
            "selected-after-reload",
            "selected-after-reload"
        ]
    );
    for request in requests {
        assert_eq!(
            request["messages"].as_array().unwrap().last().unwrap()["role"],
            "user"
        );
        assert_eq!(request["stream"], true);
    }
}

/// 【插件测试】【子任务模型】子代理中的插件模型请求继承子代理已选客户端，不退回注册时默认值。
#[tokio::test]
async fn subagent_plugin_requests_use_the_selected_child_model() {
    let fixture = ModelFixture::start(vec![
        ModelReply::delta(
            json!({"role":"assistant", "tool_calls":[tool_call(0, "lua__model__run", json!({}))]}),
        ),
        ModelReply::text("plugin model response"),
        ModelReply::text("child completed"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let runner = SubagentRunner::new(
        fixture.client("chosen-child-model", &paths),
        "Run fixture tool",
        registry(&fixture, &paths),
        SubagentProgress::new(ToolProgress::default(), ProgressMode::Hidden, false),
    );
    let (result, stats) = runner.run("Query the plugin model").await.unwrap();
    assert_eq!(result.content, "child completed");
    assert_eq!(stats.tool_calls, 1);
    assert_eq!(stats.tool_ok, 1);
    let requests = fixture.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests
        .iter()
        .all(|request| request["model"] == "chosen-child-model"));
    assert_eq!(requests[1]["messages"][0]["content"], "fixture request");
}

/// 【插件测试】【工具建议】宿主只返回模型建议；显式调用才执行，并将供应商搜索别名恢复为授权名称。
#[tokio::test]
async fn model_suggestions_require_explicit_execution_and_preserve_search_aliases() {
    let reply = || {
        ModelReply::delta(
            json!({"role":"assistant", "tool_calls":[tool_call(0, "sai_web_search", json!({"query":"fixture"}))]}),
        )
    };
    let fixture = ModelFixture::start(vec![reply(), reply()]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let mut tools = ToolRegistry::new();
    tools.set_plugin_model_client(&fixture.client("model", &paths));
    tools.register(ToolSpec::new(
        "web_search",
        "Search fixture",
        json!({"type":"object"}),
        move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("search result".into()) }
        },
    ));
    register_service(
        &mut tools,
        "suggestion",
        r#"
        sai.register_tool({name='run',description='Ask for a tool',parameters={type='object'},execute=function(args)
            local result=sai.model.complete({messages={{role='user',content='search'}},tools={'web_search'}})
            if args.execute then
                local call=result.tool_calls[1]
                return sai.tools.call(call.name, call.arguments)
            end
            return result
        end})
    "#,
        json!({"model":true, "tools":["web_search"]}),
    );
    let result: Value =
        serde_json::from_str(&tools.call("lua__suggestion__run", "{}").await.unwrap()).unwrap();
    assert_eq!(result["tool_calls"][0]["name"], "web_search");
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(
        tools
            .call("lua__suggestion__run", r#"{"execute":true}"#)
            .await
            .unwrap(),
        "search result"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// 【插件测试】【内部别名】模型不能把未公开的内部执行别名转换为普通命令，避免丢失可信 shell 语义。
#[tokio::test]
async fn private_execution_aliases_are_not_promoted_to_authorized_tool_names() {
    let fixture = ModelFixture::start(vec![ModelReply::delta(
        json!({"role":"assistant", "tool_calls":[
            tool_call(0, "__sai_dsh_bash", json!({"command":"fixture"}))
        ]}),
    )])
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let mut tools = ToolRegistry::new();
    tools.set_plugin_model_client(&fixture.client("model", &paths));
    tools.register(
        ToolSpec::new(
            "run_command",
            "Inspect command",
            json!({"type":"object"}),
            move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                async { Ok("executed".into()) }
            },
        )
        .writes(),
    );
    register_service(
        &mut tools,
        "private_alias",
        r#"
        sai.register_tool({name='run',description='Check private alias',access='writes',parameters={type='object'},execute=function()
            local result=sai.model.complete({messages={{role='user',content='command'}},tools={'run_command'}})
            local call=result.tool_calls[1]
            local ok, output=pcall(sai.tools.call, call.name, call.arguments)
            return {name=call.name, ok=ok, output=tostring(output)}
        end})
    "#,
        json!({"model":true, "tools":["run_command"]}),
    );
    let result: Value =
        serde_json::from_str(&tools.call("lua__private_alias__run", "{}").await.unwrap()).unwrap();
    assert_eq!(result["name"], "__sai_dsh_bash");
    assert_eq!(result["ok"], false);
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
