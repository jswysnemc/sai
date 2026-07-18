use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::state::active_state_dir;
use crate::tools::todo::{TodoHistoryBatch, TodoItem, TodoStore};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct TodoSnapshot {
    items: Vec<TodoItem>,
    history: Vec<TodoHistoryBatch>,
}

/// 返回 TODO 管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/todos", get(list))
}

/// 列出当前活动会话的 TODO 项与历史计划。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<TodoSnapshot>> {
    let store = store(&state)?;
    Ok(Json(TodoSnapshot {
        items: store.list().map_err(WebError::from)?,
        history: store.list_history().map_err(WebError::from)?,
    }))
}

/// 构造当前活动会话 TODO 存储。
fn store(state: &WebAppState) -> WebResult<TodoStore> {
    Ok(TodoStore::new(
        active_state_dir(&state.paths)
            .map_err(WebError::from)?
            .join("todos.json"),
    ))
}
