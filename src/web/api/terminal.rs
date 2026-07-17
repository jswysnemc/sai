use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::terminal;
use crate::config::AppConfig;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, State};
use axum::response::Response;
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct CreateTerminalRequest {
    cols: Option<u16>,
    rows: Option<u16>,
}

#[derive(Deserialize)]
struct RenameTerminalRequest {
    title: String,
}

/// 返回 PTY 终端路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/terminals", get(list).post(create))
        .route("/api/terminals/:id", patch(rename).delete(remove))
        .route("/api/terminals/:id/socket", get(socket))
}

/// 更新终端标签标题。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 终端 ID
/// - `request`: 新标题
///
/// 返回:
/// - 更新后的终端摘要
async fn rename(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<RenameTerminalRequest>,
) -> WebResult<Json<Value>> {
    let terminal = state
        .terminals
        .rename(&id, &request.title)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!(terminal)))
}

/// 列出当前终端。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let terminals = state.terminals.list().map_err(WebError::from)?;
    Ok(Json(json!({ "terminals": terminals })))
}

/// 创建工作区终端。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<CreateTerminalRequest>,
) -> WebResult<Json<Value>> {
    let workspace = state.workspaces.active().map_err(WebError::from)?;
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let terminal = state
        .terminals
        .create(
            std::path::Path::new(&workspace.path),
            &config.terminal.shell,
            request.cols.unwrap_or(100),
            request.rows.unwrap_or(30),
        )
        .map_err(WebError::from)?;
    Ok(Json(json!(terminal)))
}

/// 终止并移除终端。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<Value>> {
    let removed = state.terminals.remove(&id).map_err(WebError::from)?;
    Ok(Json(json!({ "removed": removed })))
}

/// 升级为 PTY WebSocket。
async fn socket(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> WebResult<Response> {
    let session = state
        .terminals
        .get(&id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    Ok(upgrade.on_upgrade(move |socket| terminal::serve_socket(socket, session)))
}
