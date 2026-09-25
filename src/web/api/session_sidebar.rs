use super::super::app_state::WebAppState;
use super::super::error::WebError;
use crate::state::{load_sidebar_index, patch_sidebar_index, SidebarIndexPatch};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

/// 返回侧栏索引路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/session-sidebar", get(read).patch(update))
}

/// 读取置顶、归档、未读和分组。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 侧栏索引
async fn read(
    State(state): State<WebAppState>,
) -> Result<Json<crate::state::SidebarIndex>, WebError> {
    load_sidebar_index(&state.paths)
        .map(Json)
        .map_err(WebError::from)
}

/// 合并侧栏索引的局部更新。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `patch`: 要写入的字段
///
/// 返回:
/// - 合并后的索引
async fn update(
    State(state): State<WebAppState>,
    Json(patch): Json<SidebarIndexPatch>,
) -> Result<Json<crate::state::SidebarIndex>, WebError> {
    patch_sidebar_index(&state.paths, patch)
        .map(Json)
        .map_err(WebError::from)
}
