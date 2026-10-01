//! 首次供应商引导接口，复用 TUI 的配置校验与持久化逻辑。

use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::{load_redacted, SECRET_SENTINEL};
use crate::config::onboarding::{complete_provider_setup, ProviderSetupInput};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};

/// 【首次配置】【接口注册】注册需要通过 Web 认证的供应商引导保存接口。
/// @returns 引导路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/onboarding/provider", post(complete))
}

/// 【首次配置】【保存接口】保存用户确认的供应商，成功响应始终使用脱敏配置。
/// @param state 为应用状态；input 为用户提交的连接配置
/// @returns 与配置查询相同的脱敏响应
async fn complete(
    State(state): State<WebAppState>,
    Json(input): Json<ProviderSetupInput>,
) -> WebResult<Json<Value>> {
    complete_provider_setup(&state.paths, input)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let config = load_redacted(&state.paths).map_err(WebError::from)?;
    Ok(Json(
        json!({ "config": config, "secret_sentinel": SECRET_SENTINEL }),
    ))
}
