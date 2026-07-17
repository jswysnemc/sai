use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::permission::{decide_permission, pending_permissions, PermissionDecision};
use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct PermissionDecisionRequest {
    decision: String,
    reply: Option<String>,
}

/// 返回权限审批路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/permissions/session/:id", get(list))
        .route("/api/permissions/:id/decision", post(decide))
}

/// 返回会话当前等待处理的权限请求。
async fn list(Path(id): Path<String>) -> Json<Vec<crate::permission::PermissionRequest>> {
    Json(pending_permissions(&id))
}

/// 提交工具权限决定。
async fn decide(
    Path(id): Path<String>,
    Json(request): Json<PermissionDecisionRequest>,
) -> WebResult<Json<Value>> {
    let decision = match request.decision.as_str() {
        "allow" => PermissionDecision::Allow,
        "deny" => PermissionDecision::Deny {
            reply: request.reply,
        },
        _ => return Err(WebError::bad_request("unsupported permission decision")),
    };
    decide_permission(&id, decision).map_err(WebError::from)?;
    Ok(Json(json!({ "accepted": true })))
}
