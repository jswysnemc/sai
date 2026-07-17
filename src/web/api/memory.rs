use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::memory::MemoryStore;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct RememberRequest {
    content: String,
    #[serde(default = "default_source")]
    source: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
    #[serde(default)]
    forgotten: bool,
}

fn default_source() -> String {
    "web".to_string()
}

/// 记忆管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/memory/stats", get(stats))
        .route("/api/memory/entries", get(list).post(remember))
        .route("/api/memory/search", get(search))
        .route("/api/memory/entries/:kind/:id", delete(remove))
        .route("/api/memory/reset", post(reset))
}

fn store(state: &WebAppState) -> MemoryStore {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    MemoryStore::new(&config, &state.paths)
}

async fn stats(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    Ok(Json(store(&state).stats().map_err(WebError::from)?))
}

async fn list(
    State(state): State<WebAppState>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    Ok(Json(
        store(&state)
            .list_entries(query.limit.unwrap_or(100))
            .map_err(WebError::from)?,
    ))
}

async fn remember(
    State(state): State<WebAppState>,
    Json(request): Json<RememberRequest>,
) -> WebResult<Json<Value>> {
    let id = store(&state)
        .remember_fact(&request.content, &request.source)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true, "id": id })))
}

async fn search(
    State(state): State<WebAppState>,
    Query(query): Query<SearchQuery>,
) -> WebResult<Json<Value>> {
    Ok(Json(
        store(&state)
            .recall_memories(&query.q, query.limit.unwrap_or(20), query.forgotten)
            .map_err(WebError::from)?,
    ))
}

async fn remove(
    State(state): State<WebAppState>,
    Path((kind, id)): Path<(String, i64)>,
) -> WebResult<Json<Value>> {
    let deleted = store(&state)
        .delete_entry(&kind, id)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "deleted": deleted })))
}

async fn reset(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    store(&state).reset_all(false).map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true })))
}
