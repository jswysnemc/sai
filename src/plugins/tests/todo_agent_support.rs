use super::services_support::{tool_call, ModelFixture, ModelReply};
use crate::{
    agent::{Agent, AgentMode},
    paths::SaiPaths,
    plugins::{private::PrivatePluginHost, registry::register_descriptor},
    state::StateStore,
    tools::{ToolRegistry, ToolSpec},
};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

/// 【待办 Agent 测试】【正式装配】加载真实待办包及无副作用的串行工具
/// @param root 隔离目录；model 为本地模型；mode 为运行模式；excluded 为工具白名单排除
/// @returns Agent 和完整会话状态
pub(super) fn agent(
    root: &Path,
    model: &ModelFixture,
    mode: AgentMode,
    excluded: bool,
) -> (Agent, StateStore) {
    let paths = SaiPaths::for_tests(root);
    let config = model.config("todo-model");
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let descriptor = crate::plugins::discover(&config, &paths)
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == "todo")
        .unwrap();
    let host = Arc::new(PrivatePluginHost::for_descriptor(&paths, &descriptor).unwrap());
    let mut registry = ToolRegistry::new();
    registry.configure_plugin_model(&config, &paths);
    register_descriptor(&mut registry, descriptor, host, mode == AgentMode::Plan).unwrap();
    registry.register(ToolSpec::new(
        "step",
        "Read a deterministic fixture",
        json!({"type":"object","properties":{"n":{"type":"integer"}},"required":["n"]}),
        |args| async move { Ok(format!("step {}", args["n"])) },
    ));
    if excluded {
        registry = registry.clone_filtered(&["step"]);
    }
    let agent = Agent::new(
        config,
        &paths,
        state.clone(),
        model.client("todo-model", &paths),
        registry,
        mode,
    )
    .unwrap();
    (agent, state)
}

/// 【待办 Agent 测试】【模型工具帧】单个工具独占一轮请求，避免进入并发只读分组
/// @param serial 唯一调用标识；name 为工具；arguments 为模型参数
/// @returns 真实 SSE 增量样本
pub(super) fn tool(serial: usize, name: &str, arguments: Value) -> ModelReply {
    let mut call = tool_call(0, name, arguments);
    call["id"] = json!(format!("todo-call-{serial}"));
    ModelReply::delta(json!({"role":"assistant","tool_calls":[call]}))
}

/// 【待办 Agent 测试】【提醒计数】只统计实际模型请求中出现的原中文提醒
/// @param request 捕获的请求正文
/// @returns 提醒消息数量
pub(super) fn reminder_count(request: &Value) -> usize {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| {
            message["content"]
                .as_str()
                .is_some_and(|text| text.contains("当前会话仍有未完成 TODO"))
        })
        .count()
}

/// 【待办 Agent 测试】【旧清单种子】写入原会话格式，验证 Agent 和 Web 共用兼容文件
/// @param state 当前会话
/// @returns 无
pub(super) fn seed(state: &StateStore) {
    std::fs::write(
        state.state_dir().join("todos.json"),
        json!([super::todo_support::item("a", "pending")]).to_string(),
    )
    .unwrap();
}
