use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::agent::AgentMode;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct EnterPlanRequest {
    execution_mode: String,
}

/// 【计划模式】【路由】无参数，返回独立规划入口。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/sessions/:id/plan", post(enter))
}

/// 【计划模式】【进入规划】参数为会话与普通权限模式，返回规划状态；不启动模型或执行计划。
async fn enter(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<EnterPlanRequest>,
) -> WebResult<Json<Value>> {
    let mode = AgentMode::parse(Some(&request.execution_mode))
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    if mode == AgentMode::Plan {
        return Err(WebError::bad_request(
            "execution_mode must be a normal permission mode",
        ));
    }
    if state
        .runs
        .active_runs()
        .await
        .iter()
        .any(|run| run.session_id == id)
    {
        return Err(WebError::conflict(
            "Stop the active run before entering Plan mode",
        ));
    }
    let (_, directory) = crate::state::locate_session_dirs(&state.paths, &id)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    crate::plan::store::begin(&directory, mode)
        .await
        .map_err(WebError::from)?;
    Ok(Json(json!({"mode":"plan", "execution_mode": mode.key()})))
}
