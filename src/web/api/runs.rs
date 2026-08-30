use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::runs::{QueuedRunUpdate, StartRunRequest, WebEvent, MAX_RUN_REQUEST_BYTES};
use crate::runner::ActorHandle;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Deserialize)]
struct EventQuery {
    after: Option<u64>,
}

#[derive(Deserialize)]
struct InterruptionRecoveryQuery {
    workspace_id: String,
    session_id: String,
}

/// 返回 Agent 运行路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route(
            "/api/runs",
            post(start).layer(DefaultBodyLimit::max(MAX_RUN_REQUEST_BYTES)),
        )
        .route("/api/runs/active", get(active))
        .route(
            "/api/runs/interruption-recovery",
            get(interruption_recovery),
        )
        .route("/api/runs/:id", delete(stop))
        .route(
            "/api/runs/:id/queue",
            patch(update_queue).layer(DefaultBodyLimit::max(MAX_RUN_REQUEST_BYTES)),
        )
        .route("/api/runs/:id/events", get(events))
}

/// 启动一轮流式 Agent 运行。
async fn start(
    State(state): State<WebAppState>,
    Json(request): Json<StartRunRequest>,
) -> WebResult<Json<Value>> {
    let sessions = crate::state::list_sessions(&state.paths).map_err(WebError::from)?;
    if !sessions
        .iter()
        .any(|session| session.id == request.session_id)
    {
        return Err(WebError::not_found(format!(
            "session not found: {}",
            request.session_id
        )));
    }
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let info = state
        .runs
        .start(workspace, request)
        .await
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(json!(info)))
}

/// 返回当前活动运行。
async fn active(State(state): State<WebAppState>) -> Json<Value> {
    let runs = state.runs.active_runs().await;
    Json(json!({ "run": runs.first(), "runs": runs }))
}

/// 读取并消费指定会话的无回复中断恢复输入。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 工作区和会话标识
///
/// 返回:
/// - 可选恢复运行信息
async fn interruption_recovery(
    State(state): State<WebAppState>,
    Query(query): Query<InterruptionRecoveryQuery>,
) -> WebResult<Json<Value>> {
    let run = state
        .runs
        .take_interruption_recovery(&query.workspace_id, &query.session_id)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "run": run })))
}

/// 中断指定运行。
async fn stop(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let stopped = state
        .runs
        .stop(&id)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "stopped": stopped })))
}

/// 更新排队运行内容或等待顺序。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 排队运行标识
/// - `update`: 新输入或目标位置
///
/// 返回:
/// - 更新后的运行摘要
async fn update_queue(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(update): Json<QueuedRunUpdate>,
) -> WebResult<Json<Value>> {
    let info = state
        .runs
        .update_queued(&id, update)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!(info)))
}

/// 读取 SSE 补发起始序号。
///
/// EventSource 自动重连时浏览器会上报 Last-Event-ID，它总是比连接建立时的
/// `?after=` 更新，因此优先采用，避免重连把已收到的事件再补发一遍。
///
/// 参数:
/// - `after`: 查询参数中的已接收序号
/// - `headers`: 请求头
///
/// 返回:
/// - 已接收的最后事件序号
pub(super) fn replay_after(after: Option<u64>, headers: &HeaderMap) -> u64 {
    headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .or(after)
        .unwrap_or(0)
}

/// 组装会话事件 SSE 流：先按序号补发历史，再接实时事件。
///
/// 实时段走观察者通道而不是 broadcast：慢消费者会被摘除并留下 lagged 标记，
/// 客户端重连时按最后收到的序号从落盘日志补发，不会静默丢事件。
///
/// 参数:
/// - `bus`: 会话事件总线
/// - `after`: 已接收的最后事件序号
///
/// 返回:
/// - SSE 事件流；事件总线已停止时返回空
pub(super) fn session_events_stream(
    bus: ActorHandle,
    after: u64,
) -> Option<impl Stream<Item = Result<Event, Infallible>>> {
    let journal = bus.journal();
    let subscription = bus.attach()?;
    let backlog = journal.events_after(after);
    let latest_backlog = backlog.last().map(|event| event.sequence).unwrap_or(after);
    let backlog_stream = stream::iter(
        backlog
            .into_iter()
            .map(|event| Ok::<_, Infallible>(sse_event(&event))),
    );
    let live_stream = ReceiverStream::new(subscription.events).filter_map(move |event| {
        let event = (event.sequence > latest_backlog).then_some(event);
        async move { event.map(|event| Ok::<_, Infallible>(sse_event(&event))) }
    });
    // 观察者被摘除后接收端关闭，此处补一条提示让前端知道存在空洞需要重连
    let dropped = subscription.dropped;
    let tail = stream::once(async move {
        let dropped = dropped.load(Ordering::Relaxed);
        (dropped > 0).then(|| {
            Ok::<_, Infallible>(
                Event::default()
                    .event("stream.lagged")
                    .data(json!({ "dropped": dropped }).to_string()),
            )
        })
    })
    .filter_map(|item| async move { item });
    Some(backlog_stream.chain(live_stream).chain(tail))
}

/// 订阅运行事件并按事件序号补发遗漏内容。
async fn events(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<EventQuery>,
    headers: HeaderMap,
) -> WebResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // 事件流已提升为会话级：运行标识只用于定位所属会话
    let bus = state
        .runs
        .run_bus(&id)
        .await
        .ok_or_else(|| WebError::not_found(format!("run not found: {id}")))?;
    let stream = session_events_stream(bus, replay_after(query.after, &headers))
        .ok_or_else(|| WebError::conflict("session event stream is unavailable"))?;
    Ok(Sse::new(stream).keep_alive(sse_keep_alive()))
}

/// 将 WebEvent 编码为 SSE 事件。
pub(super) fn sse_event(event: &WebEvent) -> Event {
    Event::default()
        .id(event.sequence.to_string())
        .event(event.kind.clone())
        .data(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()))
}

/// 会话事件 SSE 保活设置。
pub(super) fn sse_keep_alive() -> KeepAlive {
    KeepAlive::new()
        .interval(Duration::from_secs(15))
        .text("keep-alive")
}
