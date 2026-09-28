use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::usage_history::UsageStatsQuery;
use axum::extract::{Query, State};
use axum::routing::{delete, get};
use axum::{Json, Router};

/// 返回用量统计路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/usage/stats", get(stats))
        .route("/api/usage/logs", delete(clear))
}

/// 查询用量汇总、趋势与日志。
async fn stats(
    State(state): State<WebAppState>,
    Query(query): Query<UsageStatsQuery>,
) -> WebResult<Json<crate::usage_history::UsageStatsResponse>> {
    let response = crate::usage_history::get_stats(&state.paths, query).map_err(WebError::from)?;
    Ok(Json(response))
}

/// 清空全局用量日志。
async fn clear(State(state): State<WebAppState>) -> WebResult<Json<serde_json::Value>> {
    crate::usage_history::clear_all(&state.paths).map_err(WebError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
