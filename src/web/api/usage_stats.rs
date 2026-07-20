use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use axum::extract::{Query, State};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;

/// 返回用量统计路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/usage/stats", get(stats))
        .route("/api/usage/logs", delete(clear))
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default = "default_range")]
    range: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    provider_search: Option<String>,
    #[serde(default)]
    model_search: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

fn default_range() -> String {
    "7d".to_string()
}

/// 查询用量汇总、趋势与日志。
async fn stats(
    State(state): State<WebAppState>,
    Query(query): Query<StatsQuery>,
) -> WebResult<Json<crate::usage_history::UsageStatsResponse>> {
    let response = crate::usage_history::get_stats(
        &state.paths,
        crate::usage_history::UsageStatsQuery {
            range: query.range,
            source: query.source,
            status: query.status,
            provider_search: query.provider_search,
            model_search: query.model_search,
            limit: query.limit,
            offset: query.offset,
        },
    )
    .map_err(WebError::from)?;
    Ok(Json(response))
}

/// 清空全局用量日志。
async fn clear(State(state): State<WebAppState>) -> WebResult<Json<serde_json::Value>> {
    crate::usage_history::clear_all(&state.paths).map_err(WebError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
