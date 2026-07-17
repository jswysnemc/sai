use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspaces::WorkspaceInfo;
use axum::extract::{Path, Query, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize)]
struct WorkspaceListResponse {
    active_id: String,
    workspaces: Vec<WorkspaceInfo>,
}

#[derive(Deserialize)]
struct AddWorkspaceRequest {
    path: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct RenameWorkspaceRequest {
    name: String,
}

#[derive(Deserialize)]
struct BrowseDirectoryQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
struct CreateDirectoryRequest {
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct SwitchWorkspaceQuery {
    close_terminals: Option<bool>,
}

#[derive(Serialize)]
struct RemovedResponse {
    removed: bool,
}

/// 返回工作区管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/workspaces", get(list).post(add))
        .route("/api/workspaces/browse", get(browse))
        .route(
            "/api/workspaces/browse/directory",
            post(create_browse_directory),
        )
        .route("/api/workspaces/:id", patch(rename).delete(remove))
        .route("/api/workspaces/:id/switch", post(switch))
}

/// 列出工作区和当前活动项。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<WorkspaceListResponse>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let workspaces = state.workspaces.list().map_err(WebError::from)?;
    Ok(Json(WorkspaceListResponse {
        active_id: active.id,
        workspaces,
    }))
}

/// 添加工作区。
async fn add(
    State(state): State<WebAppState>,
    Json(request): Json<AddWorkspaceRequest>,
) -> WebResult<Json<WorkspaceInfo>> {
    let workspace = state
        .workspaces
        .add(&PathBuf::from(request.path), request.name.as_deref())
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(workspace))
}

/// 浏览服务端允许选择的目录。
async fn browse(
    Query(query): Query<BrowseDirectoryQuery>,
) -> WebResult<Json<super::super::workspaces::DirectoryListing>> {
    let listing = super::super::workspaces::browse_directories(query.path.as_deref())
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(listing))
}

/// 在允许浏览的目录下创建子目录。
///
/// 参数:
/// - `request`: 父目录路径与新目录名
///
/// 返回:
/// - 新目录的目录条目
async fn create_browse_directory(
    Json(request): Json<CreateDirectoryRequest>,
) -> WebResult<Json<super::super::workspaces::DirectoryEntry>> {
    let entry = super::super::workspaces::create_directory(&request.path, &request.name)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 重命名工作区。
async fn rename(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<RenameWorkspaceRequest>,
) -> WebResult<Json<WorkspaceInfo>> {
    let workspace = state
        .workspaces
        .rename(&id, &request.name)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(workspace))
}

/// 切换活动工作区。
///
/// 参数:
/// - `id`: 目标工作区 ID
/// - `query`: 可选 `close_terminals=true` 时先关闭全部终端
///
/// 返回:
/// - 切换后的工作区信息
async fn switch(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Query(query): Query<SwitchWorkspaceQuery>,
) -> WebResult<Json<WorkspaceInfo>> {
    // 1. Agent 运行已经绑定启动时工作目录，切换不会改变旧运行作用域
    // 2. 按请求先关闭全部终端，否则有终端时拒绝
    if query.close_terminals.unwrap_or(false) {
        close_all_terminals(&state)?;
    }
    ensure_workspace_switch_allowed(&state).await?;
    let workspace = state
        .workspaces
        .switch(&id)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(workspace))
}

/// 校验当前状态是否允许切换工作区。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 允许切换时返回成功
async fn ensure_workspace_switch_allowed(state: &WebAppState) -> WebResult<()> {
    if state.terminals.has_sessions().map_err(WebError::from)? {
        return Err(WebError::conflict(
            "close active terminals before switching workspace",
        ));
    }
    Ok(())
}

/// 关闭全部终端会话。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 全部关闭时返回成功
fn close_all_terminals(state: &WebAppState) -> WebResult<()> {
    // 1. 列出当前全部终端
    let terminals = state.terminals.list().map_err(WebError::from)?;
    // 2. 逐个终止并移除
    for terminal in terminals {
        state
            .terminals
            .remove(&terminal.id)
            .map_err(WebError::from)?;
    }
    Ok(())
}

/// 移除非活动工作区。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<RemovedResponse>> {
    let removed = state
        .workspaces
        .remove(&id)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(RemovedResponse { removed }))
}
