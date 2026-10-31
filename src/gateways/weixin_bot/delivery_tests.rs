use super::*;
use axum::{extract::State, routing::post, Json, Router};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Responses {
    requests: Arc<Mutex<Vec<Value>>>,
    replies: Arc<Mutex<std::collections::VecDeque<Value>>>,
    update: Option<(SaiPaths, ChannelContext)>,
}

/// 【微信网关】【测试接口】参数 state/payload 为响应脚本与请求，返回预设响应并模拟入站刷新。
async fn respond(State(state): State<Responses>, Json(payload): Json<Value>) -> Json<Value> {
    state.requests.lock().unwrap().push(payload);
    let response = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or(json!({"ret":0}));
    if prepare_failed(&response) {
        if let Some((paths, context)) = &state.update {
            channel_context::save_latest_channel_context(paths, context).unwrap();
        }
    }
    Json(response)
}

/// 【微信网关】【测试服务】持有模拟接口与请求记录，析构时终止服务任务。
struct Mock {
    client: WeixinBotClient,
    requests: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Mock {
    /// 【微信网关】【测试启动】参数 replies/update 为响应和模拟刷新，返回隔离本地服务。
    async fn start(replies: Vec<Value>, update: Option<(SaiPaths, ChannelContext)>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = Responses {
            requests: requests.clone(),
            replies: Arc::new(Mutex::new(replies.into())),
            update,
        };
        let app = Router::new()
            .route("/ilink/bot/sendmessage", post(respond))
            .with_state(state);
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            client: WeixinBotClient::new(url.clone(), url, "fixture".into(), None, false),
            requests,
            task,
        }
    }
}

impl Drop for Mock {
    /// 【微信网关】【测试清理】无参数，返回无；终止本地 HTTP 服务。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【微信网关】【测试消息】无参数，返回包含固定接收人、旧凭证及唯一 ID 的消息。
fn payload() -> Value {
    json!({"msg":{"to_user_id":"user", "context_token":"old", "client_id":"same-id", "item_list":[{"type":1}]}})
}

/// 【微信网关】【发送前刷新回归】无参数，返回无；缓存比调用绑定更新时直接使用新凭证。
#[tokio::test]
async fn cached_new_context_is_used_before_the_first_send() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    channel_context::save_latest_channel_context(
        &paths,
        &ChannelContext::weixin("user".into(), Some("fresh".into())),
    )
    .unwrap();
    let mock = Mock::start(vec![], None).await;
    send_with_wait(
        &mock.client,
        payload(),
        Some(&paths),
        Duration::from_millis(200),
    )
    .await
    .unwrap();
    let requests = mock.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["msg"]["context_token"], "fresh");
}

/// 【微信网关】【凭证拒绝回归】无参数，返回无；服务明确拒绝后，仅同接收人新凭证触发一次重试。
#[tokio::test]
async fn prepare_failed_retries_once_with_new_context_and_same_client_id() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mock = Mock::start(
        vec![
            json!({"ret":-2,"errmsg":"prepare failed"}),
            json!({"ret":0}),
        ],
        Some((
            paths.clone(),
            ChannelContext::weixin("user".into(), Some("fresh".into())),
        )),
    )
    .await;
    send_with_wait(
        &mock.client,
        payload(),
        Some(&paths),
        Duration::from_millis(200),
    )
    .await
    .unwrap();
    let requests = mock.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["msg"]["context_token"], "old");
    assert_eq!(requests[1]["msg"]["context_token"], "fresh");
    assert_eq!(
        requests[0]["msg"]["client_id"],
        requests[1]["msg"]["client_id"]
    );
}

/// 【微信网关】【无刷新回归】无参数，返回无；其他接收人的凭证不能触发重试，旧凭证不重复发送。
#[tokio::test]
async fn another_recipient_context_does_not_trigger_retry() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mock = Mock::start(
        vec![json!({"ret":-2,"errmsg":"prepare failed"})],
        Some((
            paths.clone(),
            ChannelContext::weixin("other".into(), Some("fresh".into())),
        )),
    )
    .await;
    let error = send_with_wait(
        &mock.client,
        payload(),
        Some(&paths),
        Duration::from_millis(150),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("no refreshed context"));
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
}

/// 【微信网关】【重试上限回归】无参数，返回无；新凭证仍遭拒绝时停止，不无限循环。
#[tokio::test]
async fn second_rejection_stops_and_keeps_server_error() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let rejection = json!({"ret":-2,"errmsg":"prepare failed"});
    let mock = Mock::start(
        vec![rejection.clone(), rejection],
        Some((
            paths.clone(),
            ChannelContext::weixin("user".into(), Some("fresh".into())),
        )),
    )
    .await;
    let error = send_with_wait(
        &mock.client,
        payload(),
        Some(&paths),
        Duration::from_millis(200),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("prepare failed"));
    assert_eq!(mock.requests.lock().unwrap().len(), 2);
}

/// 【微信网关】【普通拒绝回归】无参数，返回无；其他 ret=-2 错误也不当作凭证过期重试。
#[tokio::test]
async fn unrelated_rejection_is_not_retried() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mock = Mock::start(vec![json!({"ret":-2,"errmsg":"another cause"})], None).await;
    assert!(send_with_wait(
        &mock.client,
        payload(),
        Some(&paths),
        Duration::from_millis(200)
    )
    .await
    .is_err());
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
}

/// 【微信网关】【会话隔离回归】无参数，返回无；最近聊天属于其他人时使用当前人的独立会话缓存。
#[test]
fn recipient_specific_session_context_survives_another_recent_chat() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    crate::gateways::session::ensure_gateway_session(
        &paths,
        &ChannelContext::weixin("user".into(), Some("own".into())),
    )
    .unwrap();
    channel_context::save_latest_channel_context(
        &paths,
        &ChannelContext::weixin("other".into(), Some("wrong".into())),
    )
    .unwrap();
    assert_eq!(
        recipient_token(&paths, "user").unwrap().as_deref(),
        Some("own")
    );
}

/// 【微信网关】【媒体重试回归】无参数，返回无；媒体失败仅重发媒体项，不重复已经成功的说明文本。
#[tokio::test]
async fn media_item_retry_does_not_repeat_successful_caption() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mock = Mock::start(
        vec![
            json!({"ret":0}),
            json!({"ret":-2,"errmsg":"prepare failed"}),
            json!({"ret":0}),
        ],
        Some((
            paths.clone(),
            ChannelContext::weixin("user".into(), Some("fresh".into())),
        )),
    )
    .await;
    let client = mock.client.clone().with_delivery_paths(paths);
    client
        .send_text("user", "caption", Some("old"))
        .await
        .unwrap();
    client
        .send_message_item("user", json!({"type":2,"image_item":{}}), Some("old"))
        .await
        .unwrap();
    let requests = mock.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0]["msg"]["item_list"][0]["text_item"]["text"],
        "caption"
    );
    assert_eq!(requests[1]["msg"]["item_list"][0]["type"], 2);
    assert_eq!(requests[2]["msg"]["item_list"][0]["type"], 2);
    assert_eq!(
        requests[1]["msg"]["client_id"],
        requests[2]["msg"]["client_id"]
    );
}
