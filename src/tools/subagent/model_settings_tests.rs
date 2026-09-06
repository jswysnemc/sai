use super::{build_subagent_client, runner_factory, SubagentContext, ToolRegistry};
use crate::config::{AgentProfile, AppConfig, ProviderConfig, SubagentModelChoice};
use crate::paths::SaiPaths;
use serde_json::{json, Value};
use std::path::Path;

/// 【子任务】【模型测试】构造独立供应商和已注册档案，不读取用户配置。
/// 参数：`root` 为临时目录；返回：可直接构造客户端的子任务上下文。
fn context(root: &Path) -> SubagentContext {
    let mut provider = ProviderConfig::new_openai_compatible();
    provider.id = "test-provider".into();
    provider.enabled = true;
    provider.base_url = "http://127.0.0.1:9/v1".into();
    provider.api_key = Some("test-key".into());
    provider.default_model = "main-model".into();
    provider.thinking_level = "high".into();
    provider.models = [
        "main-model",
        "agent-model",
        "shared-model",
        "override-model",
        "updated-model",
    ]
    .map(str::to_string)
    .to_vec();
    let profile = AgentProfile {
        id: "general".into(),
        name: "Test Agent".into(),
        provider_id: provider.id.clone(),
        model: "agent-model".into(),
        register_to_main: true,
        ..Default::default()
    };
    SubagentContext {
        config: AppConfig {
            active_provider: provider.id.clone(),
            providers: vec![provider],
            agents: vec![profile],
            ..Default::default()
        },
        paths: SaiPaths::for_tests(root),
        tools: ToolRegistry::new(),
        owner_key: "model-settings-test".into(),
        session_id: "model-settings-session".into(),
    }
}

/// 【子任务】【模型测试】为测试模型创建可保存的完整选择。
/// 参数：`model` 为模型名称；返回：测试供应商上的模型选择。
fn choice(model: &str) -> SubagentModelChoice {
    SubagentModelChoice {
        provider_id: "test-provider".into(),
        model: model.into(),
        ..Default::default()
    }
}

/// 【子任务】【模型测试】读取实际构造客户端使用的模型。
/// 参数：`context` 为运行上下文，`id` 为子任务类型；返回：客户端绑定的模型。
fn client_model(context: &SubagentContext, id: &str) -> String {
    let profile = context.config.resolve_registered_agent(Some(id)).unwrap();
    build_subagent_client(context, &profile)
        .unwrap()
        .model()
        .to_string()
}

/// 【子任务】【模型测试】验证类型覆盖、档案、共享默认和主对话的完整优先级。
/// 参数：无；返回：无。
#[test]
fn model_priority_reaches_the_subagent_client() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    context
        .config
        .set_subagent_model_choice(None, choice("shared-model"))
        .unwrap();
    assert_eq!(client_model(&context, "general"), "agent-model");
    assert_eq!(client_model(&context, "explore"), "shared-model");

    context
        .config
        .set_subagent_model_choice(Some("general"), choice("override-model"))
        .unwrap();
    assert_eq!(client_model(&context, "general"), "override-model");

    // 1. 显式继承跳过档案固定模型，而不是删除类型覆盖后恢复档案模型
    context
        .config
        .set_subagent_model_choice(Some("general"), SubagentModelChoice::default())
        .unwrap();
    assert_eq!(client_model(&context, "general"), "shared-model");
    context
        .config
        .set_subagent_model_choice(None, SubagentModelChoice::default())
        .unwrap();
    assert_eq!(client_model(&context, "general"), "main-model");
    assert_eq!(
        context.config.provider(None).unwrap().default_model,
        "main-model"
    );
    assert_eq!(context.config.agents[0].model, "agent-model");
}

/// 【子任务】【模型测试】运行中保存共享设置后，新子任务读取新值并保留主对话覆盖。
/// 参数：无；返回：无。
#[test]
fn saved_subagent_choices_refresh_without_resetting_the_conversation_model() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    let mut saved = context.config.clone();
    saved.providers[0].default_model = "updated-model".into();
    saved.providers[0].thinking_level = "low".into();
    saved
        .set_subagent_model_choice(None, choice("shared-model"))
        .unwrap();
    saved
        .set_subagent_model_choice(Some("general"), choice("override-model"))
        .unwrap();
    saved.save(&context.paths).unwrap();

    runner_factory::refresh_model_settings(&mut context).unwrap();
    assert_eq!(client_model(&context, "general"), "override-model");
    assert_eq!(client_model(&context, "explore"), "shared-model");
    assert_eq!(
        context.config.provider(None).unwrap().default_model,
        "main-model"
    );
    assert_eq!(
        context.config.provider(None).unwrap().thinking_level,
        "high"
    );
}

/// 【子任务】【模型测试】已运行客户端保持原模型，新客户端应读取刚保存的档案模型。
/// 参数：无；返回：无。
#[test]
fn newly_started_subagents_use_the_latest_agent_profile_model() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    let profile = context
        .config
        .resolve_registered_agent(Some("general"))
        .unwrap();
    let existing = build_subagent_client(&context, &profile).unwrap();
    let mut saved = context.config.clone();
    saved.set_agent_model("general", "test-provider", "updated-model");
    saved.save(&context.paths).unwrap();
    assert_eq!(
        AppConfig::load(&context.paths).unwrap().agents[0].model,
        "updated-model"
    );

    runner_factory::refresh_model_settings(&mut context).unwrap();
    assert_eq!(client_model(&context, "general"), "updated-model");
    assert_eq!(existing.model(), "agent-model");
}

/// 【子任务】【模型测试】内置档案首次保存模型后，新子任务不能继续使用旧内置回退。
/// 参数：无；返回：无。
#[test]
fn newly_configured_builtin_models_refresh_before_start() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    let mut saved = context.config.clone();
    saved.set_agent_model("explore", "test-provider", "updated-model");
    saved.save(&context.paths).unwrap();

    runner_factory::refresh_model_settings(&mut context).unwrap();
    assert_eq!(client_model(&context, "explore"), "updated-model");
}

/// 【子任务】【模型测试】通过本地接口验证共享模型进入 HTTP 请求，不调用真实供应商。
/// 参数：无；返回：无。
#[tokio::test]
async fn shared_model_is_sent_in_the_actual_chat_request() {
    let root = tempfile::tempdir().unwrap();
    let mut context = context(root.path());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, mut received) = tokio::sync::mpsc::unbounded_channel();
    let app = axum::Router::new().route("/v1/chat/completions", axum::routing::post(
        move |axum::Json(body): axum::Json<Value>| {
            let sender = sender.clone();
            async move {
                sender.send(body).unwrap();
                let response = json!({"choices":[{"index":0,"delta":{"content":"model verified"},"finish_reason":"stop"}]});
                ([("content-type", "text/event-stream")], format!("data: {response}\n\ndata: [DONE]\n\n"))
            }
        },
    ));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    context.config.providers[0].base_url = format!("http://{address}/v1");
    context.config.providers[0].protocol = "openai-chat".into();
    context
        .config
        .set_subagent_model_choice(None, choice("shared-model"))
        .unwrap();
    let profile = context
        .config
        .resolve_registered_agent(Some("explore"))
        .unwrap();
    let client = build_subagent_client(&context, &profile).unwrap();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.chat_stream(Vec::new(), Vec::new(), |_| Ok(())),
    )
    .await;
    server.abort();
    assert_eq!(result.unwrap().unwrap().content, "model verified");
    assert_eq!(received.try_recv().unwrap()["model"], "shared-model");
    assert_eq!(
        context.config.provider(None).unwrap().default_model,
        "main-model"
    );
}
