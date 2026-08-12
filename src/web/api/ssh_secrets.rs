use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::ssh::{pending_ssh_secrets, submit_secret, SecretResponse};
use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

/// 提交交互征询应答的请求体。
///
/// 三种互斥输入：`cancelled` 取消；`confirmed` 用于指纹/高危确认；`secret` 用于
/// 口令/密码。秘密仅在此请求体里一次性传输，直达后端等待中的工具，绝不广播或入模型。
#[derive(Deserialize)]
struct SubmitSecretRequest {
    /// 口令 / 密码明文（仅秘密输入类使用）
    #[serde(default)]
    secret: Option<String>,
    /// 确认类应答（指纹确认 / 高危命令确认）
    #[serde(default)]
    confirmed: Option<bool>,
    /// 是否取消本次征询
    #[serde(default)]
    cancelled: bool,
}

/// 返回 SSH 交互征询路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/ssh-secrets/session/:id", get(list))
        .route("/api/ssh-secrets/:id/submit", post(submit))
}

/// 返回会话当前等待处理的 SSH 交互征询（不含秘密）。
async fn list(Path(id): Path<String>) -> Json<Vec<crate::ssh::SecretRequest>> {
    Json(pending_ssh_secrets(&id))
}

/// 提交一次 SSH 交互征询应答。
async fn submit(
    Path(id): Path<String>,
    Json(request): Json<SubmitSecretRequest>,
) -> WebResult<Json<Value>> {
    let response = if request.cancelled {
        SecretResponse::Cancelled
    } else if let Some(confirmed) = request.confirmed {
        SecretResponse::Confirmed(confirmed)
    } else if let Some(secret) = request.secret {
        SecretResponse::Provided(secret)
    } else {
        return Err(WebError::bad_request(
            "one of secret, confirmed, or cancelled is required",
        ));
    };
    submit_secret(&id, response).map_err(WebError::from)?;
    Ok(Json(json!({ "accepted": true })))
}
