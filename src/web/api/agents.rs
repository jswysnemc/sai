use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::agent_runtime::{self, AgentRuntimeProfile};
use crate::config::AppConfig;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AgentRuntimeProfilesResponse {
    profiles: Vec<AgentRuntimeProfile>,
}

#[derive(Deserialize)]
struct UpdateAgentRuntimeRequest {
    provider_id: String,
    model: String,
    thinking_level: String,
}

/// 返回 Agent 运行参数管理路由。
///
/// 返回:
/// - Agent 列表和单档案更新路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/agents/runtime", get(list_runtime_profiles))
        .route(
            "/api/agents/:agent_id/runtime",
            axum::routing::put(update_runtime_profile),
        )
}

/// 列出输入区可快速配置的 Agent 运行参数。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 已解析的 Agent 运行参数列表
async fn list_runtime_profiles(
    State(state): State<WebAppState>,
) -> WebResult<Json<AgentRuntimeProfilesResponse>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    Ok(Json(AgentRuntimeProfilesResponse {
        profiles: agent_runtime::list_profiles(&config),
    }))
}

/// 更新指定 Agent 的模型与思考等级。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `agent_id`: Agent 标识
/// - `request`: 新运行参数
///
/// 返回:
/// - 更新后的 Agent 运行参数
async fn update_runtime_profile(
    State(state): State<WebAppState>,
    Path(agent_id): Path<String>,
    Json(request): Json<UpdateAgentRuntimeRequest>,
) -> WebResult<Json<AgentRuntimeProfile>> {
    let mut config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let profile = agent_runtime::update_profile(
        &mut config,
        &agent_id,
        &request.provider_id,
        &request.model,
        &request.thinking_level,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    config.save(&state.paths).map_err(WebError::from)?;
    Ok(Json(profile))
}
