use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::{AppConfig, SubagentModelChoice, SubagentModelSettings};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

/// 子任务模型修改请求；未指定类型时更新共享默认值。
#[derive(Deserialize)]
struct UpdateSubagentModelRequest {
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(flatten)]
    selection: SubagentModelChoice,
}

/// 【Web】【子任务模型】注册查询和更新接口，无参数，返回受保护路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/subagents/model-settings", get(load).put(update))
}

/// 读取子任务专用模型设置；参数为应用状态，返回共享默认值与类型覆盖。
async fn load(State(state): State<WebAppState>) -> WebResult<Json<SubagentModelSettings>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    Ok(Json(config.subagent_model_settings()))
}

/// 【Web】【子任务模型】保存一个设置目标，不改写主对话 Agent 档案。
///
/// 参数: `state` 为应用状态，`request` 为目标类型和选择
/// 返回: 保存后的完整子任务设置
async fn update(
    State(state): State<WebAppState>,
    Json(request): Json<UpdateSubagentModelRequest>,
) -> WebResult<Json<SubagentModelSettings>> {
    let mut config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    config
        .set_subagent_model_choice(request.profile_id.as_deref(), request.selection)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    config.save(&state.paths).map_err(WebError::from)?;
    Ok(Json(config.subagent_model_settings()))
}
