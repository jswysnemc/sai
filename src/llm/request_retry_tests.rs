use super::*;
use crate::config::{AppConfig, ProviderConfig, RetryConfig};
use crate::paths::SaiPaths;
use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Replies = Arc<Mutex<(VecDeque<(u16, &'static str, String)>, Vec<Value>)>>;

/// 【模型恢复】【测试服务】按顺序返回真实 HTTP 和 SSE 响应；参数为共享状态及请求，返回响应
async fn respond(State(state): State<Replies>, Json(request): Json<Value>) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    state.1.push(request);
    let (status, retry, body) = state.0.pop_front().expect("unexpected retry");
    (
        axum::http::StatusCode::from_u16(status).unwrap(),
        [
            ("content-type", "text/event-stream"),
            ("retry-after", retry),
        ],
        body,
    )
}

/// 【模型恢复】【测试夹具】创建仅连接本地的客户端；参数为响应序列，返回客户端与服务器状态
async fn client(
    replies: Vec<(u16, &'static str, String)>,
) -> (
    OpenAiCompatibleClient,
    Replies,
    tokio::task::JoinHandle<()>,
    tempfile::TempDir,
) {
    let state = Arc::new(Mutex::new((replies.into(), Vec::new())));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/v1/chat/completions", post(respond))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let root = tempfile::tempdir().unwrap();
    let mut provider = ProviderConfig::new_openai_compatible();
    provider.id = "retry-test".into();
    provider.base_url = format!("http://{address}/v1");
    provider.protocol = "openai-chat".into();
    provider.default_model = "fixture".into();
    provider.api_key = Some("fixture".into());
    let config = AppConfig {
        active_provider: provider.id.clone(),
        providers: vec![provider],
        ..Default::default()
    };
    (
        OpenAiCompatibleClient::from_config(&config, &SaiPaths::for_tests(root.path())).unwrap(),
        state,
        server,
        root,
    )
}

/// 【模型恢复】【响应夹具】生成完整或中断的 SSE；参数为正文、完成标识，返回传输正文
fn sse(text: &str, complete: bool) -> String {
    let value = json!({"choices":[{"delta":{"content":text},"finish_reason": if complete { Some("stop") } else { None }}]});
    format!(
        "data: {value}\n\n{}",
        if complete { "data: [DONE]\n\n" } else { "" }
    )
}

/// 【模型恢复】【断流回归】重试可生成不同内容，界面和请求历史不得混入失败片段；无参数、无返回值
#[tokio::test]
async fn interrupted_stream_rewinds_unicode_before_replaying_request() {
    let (client, state, server, _root) = client(vec![
        (200, "", sse("旧片段", false)),
        (200, "", sse("新的完整回答", true)),
    ])
    .await;
    let mut visible = "之前的回答".to_string();
    let mut resets = 0;
    let retry = RetryConfig {
        max_attempts: 2,
        initial_delay_ms: 0,
        ..Default::default()
    };
    let result = client
        .chat_stream_events_retry(
            vec![ChatMessage::plain("user", "question")],
            vec![],
            &retry,
            |event| {
                match event {
                    ChatStreamEvent::Chunk(chunk) => visible.push_str(&chunk.text),
                    ChatStreamEvent::Reset { content_chars, .. } => {
                        remove_text_suffix(&mut visible, content_chars);
                        resets += 1;
                    }
                    _ => {}
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(result.content, "新的完整回答");
    assert_eq!(visible, "之前的回答新的完整回答");
    assert_eq!(resets, 1);
    let requests = &state.lock().unwrap().1;
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["messages"], requests[1]["messages"]);
    server.abort();
}

/// 【模型恢复】【服务端退避】结构化 429 使用 Retry-After，认证失败不受正文关键词影响；无参数、无返回值
#[tokio::test]
async fn http_status_and_retry_after_control_retries() {
    let (client, state, server, _root) = client(vec![
        (429, "0", "busy".into()),
        (200, "", sse("done", true)),
    ])
    .await;
    let retry = RetryConfig {
        max_attempts: 2,
        initial_delay_ms: 60_000,
        ..Default::default()
    };
    tokio::time::timeout(
        Duration::from_secs(3),
        client.chat_stream_events_retry(vec![], vec![], &retry, |_| Ok(())),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(state.lock().unwrap().1.len(), 2);
    server.abort();
}

/// 【模型恢复】【永久错误】认证错误和宿主回调错误不得再次请求模型；无参数、无返回值
#[tokio::test]
async fn permanent_and_callback_errors_are_not_retried() {
    for callback in [false, true] {
        let reply = if callback {
            (200, "", sse("partial", true))
        } else {
            (401, "0", "timeout in invalid key".into())
        };
        let (client, state, server, _root) = client(vec![reply]).await;
        let retry = RetryConfig {
            initial_delay_ms: 0,
            ..Default::default()
        };
        let result = client
            .chat_stream_events_retry(vec![], vec![], &retry, |_| {
                if callback {
                    anyhow::bail!("connection reset in host callback")
                }
                Ok(())
            })
            .await;
        assert!(result.is_err());
        assert_eq!(state.lock().unwrap().1.len(), 1);
        server.abort();
    }
}
