//! 模型设置在请求间隙切换，保留已开始请求的客户端快照。

use super::*;
use crate::agent::AgentMode;
use crate::config::ProviderConfig;
use crate::state::StateStore;
use crate::tools::ToolRegistry;

/// 【模型设置测试】【会话夹具】参数：root 为临时目录；返回：模型 A 配置、路径和独立会话
fn fixture(root: &std::path::Path) -> (AppConfig, SaiPaths, Agent) {
    let paths = SaiPaths::for_tests(root);
    let mut provider = ProviderConfig::default_openai();
    provider.api_key = Some("test-key".into());
    provider.models = vec!["model-a".into(), "model-b".into()];
    provider.default_model = "model-a".into();
    let mut config = AppConfig {
        active_provider: provider.id.clone(),
        providers: vec![provider],
        load_instruction_files: false,
        ..Default::default()
    };
    config.memory.enabled = false;
    config.skills.enabled = false;
    config.save(&paths).unwrap();
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let agent = Agent::new(
        config.clone(),
        &paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    (config, paths, agent)
}

/// 【模型设置测试】【请求快照】参数：无；返回：无，验证已开始请求不变，下一次请求采用最新设置
#[test]
fn pending_settings_apply_at_boundary_and_keep_in_flight_client() {
    let root = tempfile::tempdir().unwrap();
    let (mut config, paths, mut agent) = fixture(root.path());
    let in_flight = agent.client.clone();
    let handle = agent.live_model_settings();
    config.providers[0].default_model = "model-b".into();
    config.providers[0].thinking_level = "low".into();
    handle.stage(config.clone(), &paths).unwrap();
    config.providers[0].thinking_level = "high".into();
    config.providers[0].set_model_context_limit_tokens_for("model-b", Some(120_000));
    handle.stage(config, &paths).unwrap();
    assert_eq!(agent.client.model(), "model-a");
    assert_eq!(in_flight.model(), "model-a");
    agent.apply_pending_model_settings().unwrap();
    assert_eq!(agent.client.model(), "model-b");
    assert_eq!(agent.config.provider(None).unwrap().thinking_level, "high");
    assert_eq!(agent.context_char_budget, 120_000);
    assert_eq!(in_flight.model(), "model-a");
    assert!(Arc::ptr_eq(&handle, &agent.live_model_settings()));
    agent.apply_pending_model_settings().unwrap();
    assert_eq!(agent.client.model(), "model-b");
}

/// 【模型设置测试】【真实请求边界】参数：无；返回：无，验证同一工具轮次中的第二次 HTTP 请求采用新模型
#[tokio::test]
async fn next_request_in_same_turn_uses_saved_model_after_first_response() {
    use axum::{routing::post, Json, Router};
    use serde_json::json;
    let root = tempfile::tempdir().unwrap();
    let (mut config, paths, mut agent) = fixture(root.path());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    // 1. 【模型设置测试】【传输模拟】首轮返回工具调用，第二轮返回最终正文
    let app = Router::new().route("/v1/chat/completions", post(move |Json(body): Json<serde_json::Value>| {
        let captured = captured.clone();
        async move {
            let first = {
                let mut requests = captured.lock().unwrap();
                requests.push(body);
                requests.len() == 1
            };
            let delta = if first {
                json!({"role":"assistant","content":"switch now","tool_calls":[{"index":0,"id":"call-echo","type":"function","function":{"name":"test_echo","arguments":"{}"}}]})
            } else { json!({"role":"assistant","content":"done"}) };
            let chunk = json!({"id":"response-test","choices":[{"index":0,"delta":delta,"finish_reason":null}]});
            let end = json!({"id":"response-test","choices":[{"index":0,"delta":{},"finish_reason":if first {"tool_calls"} else {"stop"}}]});
            ([("content-type", "text/event-stream")], format!("data: {chunk}\n\ndata: {end}\n\ndata: [DONE]\n\n"))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    config.providers[0].base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    config.session.auto_title_enabled = false;
    config.retry.max_attempts = 1;
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut tools = ToolRegistry::new();
    tools.register(crate::tools::ToolSpec::new(
        "test_echo",
        "测试请求边界",
        json!({"type":"object","properties":{}}),
        |_| async { Ok("ok".to_string()) },
    ));
    agent
        .reload(
            config.clone(),
            OpenAiCompatibleClient::from_config(&config, &paths).unwrap(),
            tools,
            AgentMode::Yolo,
        )
        .unwrap();
    config.providers[0].default_model = "model-b".into();
    let handle = agent.live_model_settings();
    let mut staged = false;
    // 2. 【模型设置测试】【流式交互】首轮正文到达时提交设置，已开始请求保持原模型
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), agent.chat_stream_with_images(
        "execute test tool", Vec::new(), None, |event| {
            if !staged && matches!(event, crate::agent::AgentEvent::Chunk(ref chunk) if chunk.kind == crate::llm::ChatStreamKind::Content) {
                handle.stage(config.clone(), &paths)?;
                staged = true;
            }
            Ok(())
        },
    )).await.unwrap();
    server.abort();
    let result = result.unwrap();
    assert_eq!(result.content, "done");
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["model"], "model-a");
    assert_eq!(requests[1]["model"], "model-b");
}
