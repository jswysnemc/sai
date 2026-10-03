use crate::agent::{Agent, AgentMode, ToolVisibility};
use crate::config::AppConfig;
use crate::llm::{ChatContent, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::state::{tool_history::ToolResultOutput, StateStore};
use crate::tools::{context_blocks::NAMES, ToolRegistry};
use serde_json::{json, Value};

/// 【上下文】【测试配置】关闭无关后台能力，保持上下文实验开启
/// 参数: 无；返回隔离测试配置
fn config() -> AppConfig {
    let mut config = AppConfig::default();
    config.context.experimental_context_blocks = true;
    config.session.auto_title_enabled = false;
    config.skills.enabled = false;
    config.plugins.memory.enabled = false;
    config.load_instruction_files = false;
    config.prompt_sections.state_contract = false;
    config.prompt_sections.mode_reminder = false;
    config
}

/// 【上下文】【测试会话】建立 Agent 并写入可压缩工具历史
/// 参数: config 为配置，paths 为隔离路径；返回 Agent
fn agent(config: AppConfig, paths: &SaiPaths) -> Agent {
    let state = StateStore::new(paths).unwrap();
    state.start_turn("old", "inspect").unwrap();
    let original = "KEEP_ORIGINAL path=/src/lib.rs status=valid\n".repeat(400);
    for index in 0..6 {
        let id = format!("old-{index}");
        state
            .record_tool_call_started("old", index + 1, &id, "read_file", "{}")
            .unwrap();
        state
            .record_tool_result_completed(
                "old",
                &id,
                true,
                ToolResultOutput {
                    result_preview: &original,
                    result_ref: None,
                    error: None,
                    original_chars: original.chars().count(),
                },
            )
            .unwrap();
    }
    state.complete_turn("old", "inspected", None).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    Agent::new(
        config,
        paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap()
}

/// 【上下文】【测试参数】返回可被实际工具执行的压缩参数
/// 参数: 无；返回 JSON
fn compression_args() -> Value {
    json!({"message_ids":["old-0"], "topic":"检查", "summary":"已检查 /src/lib.rs，配置有效。", "expected_revision":0})
}

/// 【上下文】【提醒回归】白名单关闭或显式按需工具尚未加载时，不要求模型直接调用
/// 参数: 无；返回无
#[test]
fn compression_reminders_respect_configured_tool_availability() {
    for deferred in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut config = config();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: if deferred {
                NAMES.iter().map(|name| name.to_string()).collect()
            } else {
                vec!["read_file".into()]
            },
            deferred_tools: if deferred {
                vec!["compress_context".into()]
            } else {
                Vec::new()
            },
            exclusive: true,
            ..Default::default()
        });
        let mut agent = agent(config, &paths);
        agent.context_char_budget = 100;
        let mut messages = vec![crate::llm::ChatMessage::plain(
            "user",
            "pressure ".repeat(1000),
        )];
        let mut reminded = false;
        agent
            .remind_context_blocks(&mut messages, &mut reminded)
            .unwrap();
        assert!(!reminded);
        assert_eq!(messages.len(), 1);
    }
}

/// 【上下文】【工具接入】通过注册工具提交，并验证正式请求与关闭开关后的行为
/// 参数: 无；返回无
#[tokio::test]
async fn tools_project_into_requests_and_disable_restores_originals() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(config(), &paths);
    let status: Value =
        serde_json::from_str(&agent.tools.call("context_status", "{}").await.unwrap()).unwrap();
    assert_eq!(status["revision"], 0);
    assert_eq!(status["candidates"][0]["eligible"], true);
    agent
        .tools
        .call("compress_context", &compression_args().to_string())
        .await
        .unwrap();
    agent.state.start_turn("new", "continue").unwrap();
    let messages = agent
        .chat_messages_for_turn("new", "continue", &[], None, None)
        .unwrap();
    let tool = messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("old-0"))
        .unwrap();
    assert!(
        matches!(&tool.content, Some(ChatContent::Text(text)) if text.contains("已检查") && !text.contains("KEEP_ORIGINAL"))
    );
    let restored: Value = serde_json::from_str(
        &agent
            .tools
            .call("restore_context", r#"{"message_id":"old-0","limit":100}"#)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(restored["content"]
        .as_str()
        .unwrap()
        .contains("KEEP_ORIGINAL"));
    agent.config.context.experimental_context_blocks = false;
    agent.replace_tools(ToolRegistry::new());
    assert!(NAMES.iter().all(|name| !agent.tools.contains(name)));
    let messages = agent
        .chat_messages_for_turn("new", "continue", &[], None, None)
        .unwrap();
    let tool = messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("old-0"))
        .unwrap();
    assert!(
        matches!(&tool.content, Some(ChatContent::Text(text)) if text.contains("KEEP_ORIGINAL"))
    );
}

/// 【上下文】【可见性回归】普通、渐进、Jev 和锚定模式都能直接调用实验工具
/// 参数: 无；返回无
#[test]
fn tools_are_visible_in_all_builtin_modes() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    for (progressive, jev, anchor) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (true, false, true),
    ] {
        let mut config = config();
        config.jev.routing.enabled = jev;
        if progressive && !anchor {
            config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
                deferred_tools: vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
                ..Default::default()
            });
        }
        let visibility = ToolVisibility::from_config_with_anchor(&config, anchor, false);
        let mut registry = ToolRegistry::new();
        crate::tools::context_blocks::register(&mut registry, &state, &config);
        let definitions = visibility.definitions(&registry);
        for name in NAMES {
            assert!(definitions
                .iter()
                .any(|definition| definition.function.name == name));
            assert!(visibility.is_visible(name));
            assert!(!visibility.requires_load(name));
        }
    }
    let mut registry = ToolRegistry::new();
    crate::tools::context_blocks::register(&mut registry, &state, &AppConfig::default());
    assert!(NAMES.iter().all(|name| !registry.contains(name)));
    assert!(
        !serde_json::from_str::<crate::config::ContextConfig>("{}")
            .unwrap()
            .experimental_context_blocks
    );
}

/// 【上下文】【会话切换】注册表重建和会话切换必须释放旧会话绑定
/// 参数: 无；返回无
#[tokio::test]
async fn registry_rebinds_after_mode_and_session_changes() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(config(), &paths);
    agent
        .tools
        .call("compress_context", &compression_args().to_string())
        .await
        .unwrap();
    agent
        .switch_mode(AgentMode::Plan, ToolRegistry::new())
        .unwrap();
    assert!(agent.tools.contains("compress_context"));
    agent.replace_tools(ToolRegistry::new());
    assert!(agent.tools.contains("restore_context"));
    let session = crate::state::create_session(&paths, Some("new session")).unwrap();
    agent
        .replace_state(StateStore::for_session(&paths, &session.id).unwrap())
        .unwrap();
    let status: Value =
        serde_json::from_str(&agent.tools.call("context_status", "{}").await.unwrap()).unwrap();
    assert_eq!(status["revision"], 0);
    assert_eq!(status["total_blocks"], 0);
    assert!(agent
        .tools
        .call("restore_context", r#"{"message_id":"old-0"}"#)
        .await
        .is_err());
}

type Requests = std::sync::Arc<std::sync::Mutex<Vec<Value>>>;

/// 【上下文】【本地模型】按固定轮次请求状态、压缩和回读，记录实际 HTTP 请求
/// 参数: requests 为共享记录，request 为聊天请求；返回 SSE 响应
async fn model_response(
    axum::extract::State(requests): axum::extract::State<Requests>,
    axum::Json(request): axum::Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let mut requests = requests.lock().unwrap();
    let round = requests.len();
    requests.push(request);
    let call = match round {
        0 => Some(("status", "context_status", json!({}))),
        1 => Some(("compress", "compress_context", compression_args())),
        2 => Some((
            "restore",
            "restore_context",
            json!({"message_id":"old-0", "limit":80}),
        )),
        _ => None,
    };
    let chunk = match call {
        Some((id, name, args)) => json!({"choices":[{"index":0,"delta":{"tool_calls":[{
            "index":0,"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}
        }]},"finish_reason":"tool_calls"}]}),
        None => json!({"choices":[{"index":0,"delta":{"content":"done"},"finish_reason":"stop"}]}),
    };
    (
        [("content-type", "text/event-stream")],
        format!("data: {chunk}\n\ndata: [DONE]\n\n"),
    )
        .into_response()
}

/// 【上下文】【轮次集成】使用本地 HTTP 模型验证下一请求采用摘要，随后能精确回读
/// 参数: 无；返回无
#[tokio::test]
async fn tool_rounds_send_summary_then_exact_restored_page() {
    let requests = Requests::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/v1/chat/completions", axum::routing::post(model_response))
        .with_state(requests.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let paths = SaiPaths::for_tests(root.path());
        let mut config = config();
        let mut provider = crate::config::ProviderConfig::default_openai();
        provider.base_url = format!("http://{address}/v1");
        provider.api_key = Some("test".into());
        config.active_provider = provider.id.clone();
        config.providers = vec![provider];
        let mut agent = agent(config, &paths);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            agent.chat_stream_with_image("continue", None, |_| Ok(())),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.content, "done");
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let tool_content = |round: usize, id: &str| -> String {
            requests[round]["messages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|message| message["tool_call_id"] == id)
                .unwrap()["content"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert!(tool_content(0, "old-0").contains("KEEP_ORIGINAL"));
        assert!(tool_content(1, "old-0").contains("KEEP_ORIGINAL"));
        assert!(tool_content(2, "old-0").contains("已检查"));
        assert!(!tool_content(2, "old-0").contains("KEEP_ORIGINAL"));
        assert!(tool_content(3, "restore").contains("KEEP_ORIGINAL"));
        let receipt: Value = serde_json::from_str(&tool_content(2, "compress")).unwrap();
        assert!(
            receipt.get("summary").is_none(),
            "压缩回执不应重复携带摘要正文"
        );
        assert!(receipt["block_id"].as_str().unwrap().starts_with("cb_"));
        let before = crate::token_estimate::estimate_tokens(&requests[1]["messages"].to_string());
        let after = crate::token_estimate::estimate_tokens(&requests[2]["messages"].to_string());
        assert!(
            after < before,
            "including tool traffic: {before} -> {after}"
        );
    })
    .await;
    server.abort();
}

/// 【上下文】【恢复模型】提供本地摘要响应并记录请求，避免依赖外部模型
/// 参数: requests 为记录，request 为摘要请求；返回 SSE 摘要
async fn compaction_response(
    axum::extract::State(requests): axum::extract::State<Requests>,
    axum::Json(request): axum::Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    requests.lock().unwrap().push(request);
    let chunk = json!({"choices":[{"index":0,"delta":{"content":"已检查旧源码结果，路径为 /src/lib.rs，继续处理最近四项工具结果。"},"finish_reason":"stop"}]});
    (
        [("content-type", "text/event-stream")],
        format!("data: {chunk}\n\ndata: [DONE]\n\n"),
    )
        .into_response()
}

/// 【上下文】【恢复集成】通过真实 Agent 溢出恢复入口请求摘要并继续当前长轮次
/// 参数: 无；返回无
#[tokio::test]
async fn overflow_recovery_combines_context_blocks_with_running_compaction() {
    use crate::agent::turn_request::TurnRequest;
    let requests = Requests::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/v1/chat/completions",
            axum::routing::post(compaction_response),
        )
        .with_state(requests.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let paths = SaiPaths::for_tests(root.path());
        let mut config = config();
        let mut provider = crate::config::ProviderConfig::default_openai();
        provider.base_url = format!("http://{address}/v1");
        provider.api_key = Some("test".into());
        config.active_provider = provider.id.clone();
        config.providers = vec![provider];
        let state = StateStore::new(&paths).unwrap();
        let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
        let mut agent = Agent::new(
            config,
            &paths,
            state,
            client,
            ToolRegistry::new(),
            AgentMode::Yolo,
        )
        .unwrap();
        agent.state.start_turn("running", "continue").unwrap();
        let original = "KEEP_ORIGINAL path=/src/lib.rs status=valid\n".repeat(300);
        for index in 0..16 {
            let id = format!("running-{index}");
            agent
                .state
                .record_tool_call_started_with_context(
                    "running",
                    index + 1,
                    crate::state::tool_history::ToolAssistantContext {
                        assistant_round: index + 1,
                        assistant_reasoning: None,
                    },
                    &id,
                    "read_file",
                    "{}",
                )
                .unwrap();
            agent
                .state
                .record_tool_result_completed(
                    "running",
                    &id,
                    true,
                    ToolResultOutput {
                        result_preview: &original,
                        result_ref: None,
                        error: None,
                        original_chars: original.chars().count(),
                    },
                )
                .unwrap();
        }
        let mut args = compression_args();
        args["message_ids"] = json!(["running-0"]);
        agent
            .tools
            .call("compress_context", &args.to_string())
            .await
            .unwrap();
        let mut before = agent
            .chat_messages_for_turn("running", "continue", &[], None, None)
            .unwrap();
        before.extend(
            agent
                .state
                .project_running_turn_tool_messages("running")
                .unwrap(),
        );
        agent.project_context_blocks(&mut before).unwrap();
        agent.context_char_budget = crate::state::occupancy_tokens(&before, None) / 2;
        let request = TurnRequest {
            turn_id: "running",
            input: "continue",
            image_urls: &[],
            memory_index_prompt: None,
            plugin_reply_reminder: None,
            inter_message_source: None,
            wait_for_external: false,
        };
        let recovered = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            agent.recover_after_provider_overflow(
                request,
                &before,
                &anyhow::anyhow!("maximum context length exceeded"),
                &mut |_| Ok(()),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(recovered, "已完成局部压缩的长轮次仍应能通过全局摘要恢复");
        assert!(!requests.lock().unwrap().is_empty(), "应实际请求压缩模型");
        let mut after = agent
            .chat_messages_for_turn("running", "continue", &[], None, None)
            .unwrap();
        after.extend(
            agent
                .state
                .project_running_turn_tool_messages("running")
                .unwrap(),
        );
        agent.project_context_blocks(&mut after).unwrap();
        assert_eq!(
            after
                .iter()
                .filter(|message| message.role == "tool")
                .count(),
            4
        );
        assert!(crate::state::occupancy_tokens(&after, None) < agent.context_char_budget);
        let restored = agent
            .tools
            .call(
                "restore_context",
                r#"{"message_id":"running-0","limit":80}"#,
            )
            .await
            .unwrap();
        assert!(restored.contains("KEEP_ORIGINAL"));
    })
    .await;
    server.abort();
}
