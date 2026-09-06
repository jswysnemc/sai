use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::state::{SessionTree, SessionTurnPreview, StateStore};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// 切换活动分支的请求体。
#[derive(Debug, Deserialize)]
pub(super) struct SwitchBranchRequest {
    /// 目标轮次标识
    pub turn_id: String,
}

/// 分支操作的响应体。
#[derive(Debug, Serialize)]
pub(super) struct BranchResponse {
    /// 操作后的活动叶子；已在根部时为空
    pub active_leaf_id: Option<String>,
}

/// 返回会话分支树路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 分支树查询与切换路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/sessions/:id/turn-tree", get(turn_tree))
        .route("/api/sessions/:id/turn-tree/:turn_id", get(turn_preview))
        .route("/api/sessions/:id/turn-tree/switch", post(switch_branch))
        .route("/api/sessions/:id/turn-tree/undo", post(undo_to_parent))
}

/// 【会话分支】【消息预览】返回指定轮次完整消息，保持活动分支不变。
/// 参数：`state` 为应用状态，`id` 为会话标识，`turn_id` 为轮次标识；返回：完整可见消息。
async fn turn_preview(
    State(state): State<WebAppState>,
    Path((id, turn_id)): Path<(String, String)>,
) -> WebResult<Json<SessionTurnPreview>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let preview = store
        .session_turn_preview(&turn_id)
        .map_err(WebError::from)?
        .ok_or_else(|| WebError::not_found(format!("turn not found: {turn_id}")))?;
    Ok(Json(preview))
}

/// 读取会话的完整轮次树。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 含全部分支与活动叶子的树
async fn turn_tree(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<SessionTree>> {
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let tree = store
        .session_tree()
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(tree))
}

/// 把活动叶子切换到指定轮次。
///
/// 切换只移动指针，两条分支的轮次都会保留。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
/// - `request`: 目标轮次
///
/// 返回:
/// - 切换后的活动叶子
async fn switch_branch(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<SwitchBranchRequest>,
) -> WebResult<Json<BranchResponse>> {
    // 运行中切换会让上下文与流式输出错位，先拒绝
    let _workspace_id = super::sessions::reject_session_run(&state, &id).await?;
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    store
        .switch_active_leaf(&request.turn_id)
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(BranchResponse {
        active_leaf_id: Some(request.turn_id),
    }))
}

/// 把活动叶子退回指定轮次的父轮次。
///
/// 与旧的撤销不同，这里不删除任何轮次：退出的分支随时可以切回。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
/// - `request`: 要退出的轮次
///
/// 返回:
/// - 退回后的活动叶子
async fn undo_to_parent(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<SwitchBranchRequest>,
) -> WebResult<Json<BranchResponse>> {
    let _workspace_id = super::sessions::reject_session_run(&state, &id).await?;
    let store = StateStore::for_session(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    let parent = store
        .move_leaf_to_parent(&request.turn_id)
        .map_err(|error| WebError::conflict(error.to_string()))?;
    Ok(Json(BranchResponse {
        active_leaf_id: parent,
    }))
}
