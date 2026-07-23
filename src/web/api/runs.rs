use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::runs::{StartRunRequest, WebEvent, MAX_RUN_REQUEST_BYTES};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

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

/// 订阅运行事件并按事件序号补发遗漏内容。
async fn events(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<EventQuery>,
    headers: HeaderMap,
) -> WebResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let journal = state
        .runs
        .journal(&id)
        .await
        .ok_or_else(|| WebError::not_found(format!("run not found: {id}")))?;
    let receiver = journal.subscribe();
    let after = query
        .after
        .or_else(|| {
            headers
                .get("last-event-id")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0);
    let backlog = journal.events_after(after);
    let latest_backlog = backlog.last().map(|event| event.sequence).unwrap_or(after);
    let backlog_stream = stream::iter(
        backlog
            .into_iter()
            .map(|event| Ok::<_, Infallible>(sse_event(&event))),
    );
    let live_stream = BroadcastStream::new(receiver).filter_map(move |event| {
        let event = event.ok().filter(|event| event.sequence > latest_backlog);
        async move { event.map(|event| Ok::<_, Infallible>(sse_event(&event))) }
    });
    Ok(Sse::new(backlog_stream.chain(live_stream)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// 将 WebEvent 编码为 SSE 事件。
fn sse_event(event: &WebEvent) -> Event {
    Event::default()
        .id(event.sequence.to_string())
        .event(event.kind.clone())
        .data(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()))
}
