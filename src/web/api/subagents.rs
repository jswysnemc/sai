use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::tools::subagent_state::{
    cancel_subagent, list_subagents, subagent_event_stream, subagent_snapshot, subagent_timeline,
    SubagentSnapshot,
};
use axum::extract::{Path, Query};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Deserialize)]
struct EventQuery {
    after: Option<u64>,
}

/// 返回子智能体管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/subagents", get(list))
        .route("/api/subagents/:id", get(detail))
        .route("/api/subagents/:id/events", get(events))
        .route("/api/subagents/:id/cancel", post(cancel))
}

/// 实时订阅子智能体详情变化，并补发断线期间遗漏事件。
async fn events(
    Path(id): Path<String>,
    Query(query): Query<EventQuery>,
) -> WebResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let after = query.after.unwrap_or_default();
    let (backlog, receiver) = subagent_event_stream(&id, after)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let latest = backlog.last().map(|event| event.sequence).unwrap_or(after);
    let backlog_stream = stream::iter(
        backlog
            .into_iter()
            .map(|event| Ok::<_, Infallible>(subagent_sse_event(&event))),
    );
    let live_stream = BroadcastStream::new(receiver).filter_map(move |event| {
        let event = event.ok().filter(|event| event.sequence > latest);
        async move { event.map(|event| Ok::<_, Infallible>(subagent_sse_event(&event))) }
    });
    Ok(Sse::new(backlog_stream.chain(live_stream)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// 将子智能体详情事件编码为 SSE。
///
/// 参数:
/// - `event`: 子智能体详情事件
///
/// 返回:
/// - SSE 事件
fn subagent_sse_event(event: &crate::tools::subagent_event::SubagentStreamEvent) -> Event {
    Event::default()
        .id(event.sequence.to_string())
        .event("subagent.updated")
        .data(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()))
}

/// 列出当前进程内的子智能体。
async fn list() -> Json<Vec<SubagentSnapshot>> {
    Json(list_subagents())
}

/// 返回单个子智能体的详情,附带执行时间线。
async fn detail(Path(id): Path<String>) -> WebResult<Json<Value>> {
    let snapshot =
        subagent_snapshot(&id).map_err(|error| WebError::not_found(error.to_string()))?;
    let timeline =
        subagent_timeline(&id).map_err(|error| WebError::not_found(error.to_string()))?;
    // 1. 快照字段平铺,时间线作为附加字段合并进同一响应
    let mut body = serde_json::to_value(&snapshot).map_err(anyhow::Error::from)?;
    if let Value::Object(map) = &mut body {
        map.insert(
            "timeline".to_string(),
            serde_json::to_value(&timeline).map_err(anyhow::Error::from)?,
        );
    }
    Ok(Json(body))
}

/// 取消指定子智能体。
async fn cancel(Path(id): Path<String>) -> WebResult<Json<SubagentSnapshot>> {
    Ok(Json(
        cancel_subagent(&id).map_err(|error| WebError::not_found(error.to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use crate::tools::subagent_state::create_subagent;

    #[tokio::test]
    async fn detail_returns_snapshot_with_timeline() {
        let (subagent, _cancel) =
            create_subagent("detail api".to_string(), "general".to_string(), 5);

        let response = super::detail(axum::extract::Path(subagent.id.clone()))
            .await
            .unwrap();
        let body = response.0;

        assert_eq!(body["id"], subagent.id.as_str());
        assert!(body["timeline"].is_array());
    }

    #[tokio::test]
    async fn detail_rejects_unknown_id() {
        let result = super::detail(axum::extract::Path("missing-subagent".to_string())).await;

        assert!(result.is_err());
    }
}
