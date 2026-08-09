use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::terminal;
use super::super::terminal::SshCreateOutcome;
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
    /// 指定后建立 SSH 远程会话，缺省创建本地 PTY 会话
    ssh_host_id: Option<String>,
    /// 私钥口令，仅用于本次连接，不会写入配置
    passphrase: Option<String>,
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
///
/// 指定 ssh_host_id 时建立远程会话；远端主机密钥尚未信任或已变更时不会连接，
/// 而是回传指纹供前端确认，避免把凭据发往未经核实的主机。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<CreateTerminalRequest>,
) -> WebResult<Json<Value>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let cols = request.cols.unwrap_or(100);
    let rows = request.rows.unwrap_or(30);

    // 1. 未指定 SSH 主机时创建本地 PTY 会话
    let Some(host_id) = request.ssh_host_id.filter(|id| !id.trim().is_empty()) else {
        let workspace = state.workspaces.active().map_err(WebError::from)?;
        let terminal = state
            .terminals
            .create(
                std::path::Path::new(&workspace.path),
                &config.terminal.shell,
                cols,
                rows,
            )
            .map_err(WebError::from)?;
        return Ok(Json(json!(terminal)));
    };

    // 2. 交给终端管理器建立会话，主机密钥未通过校验时回传指纹交由用户确认
    let host = config
        .ssh
        .find(&host_id)
        .ok_or_else(|| WebError::not_found(format!("ssh host not found: {host_id}")))?;
    let passphrase = request.passphrase.as_deref().filter(|value| !value.is_empty());
    match state
        .terminals
        .create_ssh(host, passphrase, cols, rows)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?
    {
        SshCreateOutcome::Created(terminal) => Ok(Json(json!(terminal))),
        SshCreateOutcome::HostKeyPending { key, status } => Ok(Json(json!({
            "host_key_prompt": super::ssh_hosts::host_key_prompt(&key, &status),
        }))),
    }
}

/// 终止并移除终端。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<Value>> {
    let removed = state.terminals.remove(&id).await.map_err(WebError::from)?;
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
