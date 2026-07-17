use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::weixin_login::WeixinLoginSnapshot;
use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct StartLoginRequest {
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    bot_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyCodeRequest {
    verify_code: String,
}

/// 返回微信扫码登录路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/gateways/weixin/login", post(start))
        .route(
            "/api/gateways/weixin/login/:session_id",
            axum::routing::get(status),
        )
        .route(
            "/api/gateways/weixin/login/:session_id/verify",
            post(verify),
        )
}

/// 发起微信扫码登录并返回二维码。
async fn start(
    State(state): State<WebAppState>,
    Json(request): Json<StartLoginRequest>,
) -> WebResult<Json<WeixinLoginSnapshot>> {
    let snapshot = state
        .weixin_login
        .start(request.base_url, request.bot_type)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(snapshot))
}

/// 查询微信扫码登录状态。
async fn status(
    State(state): State<WebAppState>,
    Path(session_id): Path<String>,
) -> WebResult<Json<WeixinLoginSnapshot>> {
    state
        .weixin_login
        .status(&session_id)
        .map(Json)
        .ok_or_else(|| WebError::not_found(format!("weixin login session not found: {session_id}")))
}

/// 向微信扫码登录会话提交验证码。
async fn verify(
    State(state): State<WebAppState>,
    Path(session_id): Path<String>,
    Json(request): Json<VerifyCodeRequest>,
) -> WebResult<Json<WeixinLoginSnapshot>> {
    if request.verify_code.trim().is_empty() {
        return Err(WebError::bad_request(
            "verify_code cannot be empty".to_string(),
        ));
    }
    state
        .weixin_login
        .submit_verify_code(&session_id, &request.verify_code)
        .map(Json)
        .ok_or_else(|| WebError::not_found(format!("weixin login session not found: {session_id}")))
}
