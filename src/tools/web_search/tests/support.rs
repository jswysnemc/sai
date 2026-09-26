use axum::{
    body::Bytes,
    extract::{Request, State},
    response::Response,
    routing::any,
    Router,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) type Captured =
    Arc<Mutex<Vec<(String, reqwest::Method, reqwest::header::HeaderMap, Vec<u8>)>>>;

/// 【网页搜索测试】【本地服务】持有响应队列与任务，释放时自动停止监听。
pub(super) struct Server {
    pub url: String,
    pub requests: Captured,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct ServerState {
    requests: Captured,
    responses: Arc<Mutex<VecDeque<Response>>>,
    delay: Duration,
}

impl Server {
    /// 【网页搜索测试】【服务启动】接收响应队列与延迟；返回回环服务及请求记录。
    pub async fn start(responses: Vec<Response>, delay: Duration) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let state = ServerState {
            requests: requests.clone(),
            responses: Arc::new(Mutex::new(responses.into())),
            delay,
        };
        let app = Router::new().fallback(any(handle)).with_state(state);
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            requests,
            task,
        }
    }
}

impl Drop for Server {
    /// 【网页搜索测试】【资源释放】无参数；中止测试监听任务。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【网页搜索测试】【请求记录】接收服务状态和请求；保存完整正文后返回下一条响应。
async fn handle(State(state): State<ServerState>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    state.requests.lock().unwrap().push((
        parts.uri.to_string(),
        parts.method,
        parts.headers,
        bytes.to_vec(),
    ));
    tokio::time::sleep(state.delay).await;
    state
        .responses
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| response(500, "unexpected request"))
}

/// 【网页搜索测试】【响应样本】接收状态码及正文；返回 UTF-8 JSON 响应。
pub(super) fn response(status: u16, body: impl Into<Bytes>) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json; charset=utf-8")
        .body(axum::body::Body::from(body.into()))
        .unwrap()
}
