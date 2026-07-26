use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::state::input_history::{append_input_history, load_input_history, INPUT_HISTORY_LIMIT};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// 输入历史响应。
#[derive(Serialize)]
struct InputHistoryResponse {
    /// 按时间正序排列，末项为最近一次输入
    entries: Vec<String>,
    /// 服务端保留的条数上限
    limit: usize,
}

/// 追加输入历史的请求体。
#[derive(Deserialize)]
struct AppendInputHistoryRequest {
    entry: String,
}

/// 返回输入历史路由。
///
/// 与 TUI 共用同一份跨会话存储，两端的上下键历史因此保持一致。
///
/// 返回:
/// - 输入历史路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/input-history", get(load).post(append))
}

/// 读取跨会话输入历史。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 历史条目与容量上限
async fn load(State(state): State<WebAppState>) -> WebResult<Json<InputHistoryResponse>> {
    let entries = load_input_history(&state.paths).map_err(WebError::from)?;
    Ok(Json(InputHistoryResponse {
        entries,
        limit: INPUT_HISTORY_LIMIT,
    }))
}

/// 追加一条输入历史并返回更新后的列表。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 待记录的输入
///
/// 返回:
/// - 更新后的历史条目
async fn append(
    State(state): State<WebAppState>,
    Json(request): Json<AppendInputHistoryRequest>,
) -> WebResult<Json<InputHistoryResponse>> {
    append_input_history(&state.paths, &request.entry).map_err(WebError::from)?;
    let entries = load_input_history(&state.paths).map_err(WebError::from)?;
    Ok(Json(InputHistoryResponse {
        entries,
        limit: INPUT_HISTORY_LIMIT,
    }))
}
