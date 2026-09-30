//! 【Jev路由】【轮次回归】通过本地 Jev 和聊天接口检查实际发送的请求。
use super::*;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

type Requests = Arc<Mutex<Vec<Value>>>;

/// 会话记忆提取后台任务的系统提示标记；它与主对话共用聊天接口。
const EXTRACTION_WORKER: &str = "session memory extraction worker";

/// 取本轮主对话发出的最后一次聊天请求，排除轮次结束后异步发起的会话记忆提取。
///
/// 参数:
/// - `requests`: 已记录的聊天请求
///
/// 返回:
/// - 主对话请求
fn last_turn_request(requests: &Requests) -> Value {
    requests
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|request| !request["messages"].to_string().contains(EXTRACTION_WORKER))
        .cloned()
        .expect("本轮应发出主对话请求")
}

/// 记录聊天请求并返回最小回复；参数为共享记录和请求，返回流式或普通 HTTP 响应。
async fn chat_response(
    State(requests): State<Requests>,
    Json(request): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if request["stream"] == true {
        requests.lock().unwrap().push(request);
        let chunk = json!({"choices": [{"index": 0, "delta": {"content": "answer"}, "finish_reason": "stop"}]});
        (
            [("content-type", "text/event-stream")],
            format!("data: {chunk}\n\ndata: [DONE]\n\n"),
        )
            .into_response()
    } else {
        Json(json!({"choices": [{"message": {"role": "assistant", "content": "{}"}, "finish_reason": "stop"}]})).into_response()
    }
}

/// 验证完整轮次中只有命中时才发送索引和契约，关闭路由恢复旧行为，无参数、无返回值。
#[tokio::test]
async fn full_turn_sends_only_selected_memory_and_prompt_context() {
    let requests = Requests::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route("/v1/chat/completions", axum::routing::post(chat_response))
        .with_state(requests.clone())
        .route(
            "/jev",
            axum::routing::post(super::jev_prompt_tests::route_response),
        );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    for (need, routing, expected) in [
        ("none", true, false),
        ("all", true, true),
        ("fail", true, false),
        ("none", false, true),
    ] {
        let temp = tempfile::tempdir().unwrap();
        // 上一轮的后台提取可能晚到，每轮开始前清空记录
        requests.lock().unwrap().clear();
        crate::runtime_cwd::scope(temp.path().to_path_buf(), async {
            let paths = SaiPaths::for_tests(temp.path());
            let mut config = AppConfig::default();
            let mut provider = crate::config::ProviderConfig::default_openai();
            provider.base_url = format!("http://{address}/v1");
            provider.api_key = Some("test".into());
            config.active_provider = provider.id.clone();
            config.providers = vec![provider];
            config.jev.routing.enabled = routing;
            config.plugins.memory.jev_injection = true;
            config.session.auto_title_enabled = false;
            config.skills.enabled = false;
            config.load_instruction_files = false;
            config.system_prompt = Some("静态<jev description=\"配置场景\">轮次条件正文</jev>".into());
            config.model_endpoints.push(serde_json::from_value(json!({
                "id": "local", "kind": "jev", "name": "Local", "endpoint": format!("http://{address}/jev"), "api_key": "test"
            })).unwrap());
            let state = StateStore::new(&paths).unwrap();
            let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
            let mut agent = Agent::new(config, &paths, state, client, ToolRegistry::new(), AgentMode::Yolo).unwrap();
            agent.memory.notes(Some(temp.path())).save(crate::memory::file_store::MemoryScope::Project, &crate::memory::file_store::MemoryEntry {
                front: crate::memory::file_store::Frontmatter {
                    name: "project-fact".into(), description: "记忆回归标记".into(), memory_type: crate::memory::file_store::MemoryType::Project,
                }, body: "仅通过记忆工具读取的正文".into(),
            }, "记忆回归标记").unwrap();
            let result = agent.chat_stream_with_image(need, None, |_| Ok(())).await.unwrap();
            assert_eq!(result.content, "answer");
            let request = last_turn_request(&requests);
            let text = serde_json::to_string(&request["messages"]).unwrap();
            assert_eq!(text.contains("轮次条件正文"), expected, "{need}/{routing}");
            assert_eq!(text.contains("记忆回归标记"), expected, "{need}/{routing}");
            assert_eq!(text.contains("write_memory"), expected, "{need}/{routing}");
            assert!(!text.contains("仅通过记忆工具读取的正文"));
            if routing {
                let system = request["messages"].as_array().unwrap().iter().filter(|m| m["role"] == "system").map(Value::to_string).collect::<String>();
                assert!(!system.contains("轮次条件正文"));
                assert!(!system.contains("write_memory"));
            }
        }).await;
    }
    server.abort();
}
