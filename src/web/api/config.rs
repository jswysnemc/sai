use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::{self, SECRET_SENTINEL};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct ConfigResponse {
    config: Value,
    secret_sentinel: &'static str,
}

/// 返回配置管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/config", get(load).put(save))
}

/// 读取脱敏配置。
async fn load(State(state): State<WebAppState>) -> WebResult<Json<ConfigResponse>> {
    let config = config_service::load_redacted(&state.paths).map_err(WebError::from)?;
    Ok(Json(ConfigResponse {
        config,
        secret_sentinel: SECRET_SENTINEL,
    }))
}

/// 校验并保存配置。
async fn save(
    State(state): State<WebAppState>,
    Json(config): Json<Value>,
) -> WebResult<Json<ConfigResponse>> {
    let config = config_service::save(&state.paths, config)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(ConfigResponse {
        config,
        secret_sentinel: SECRET_SENTINEL,
    }))
}
