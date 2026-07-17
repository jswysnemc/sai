use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::gateways::manager::{
    gateway_runtime_statuses, start_gateway, stop_gateway, GatewayRuntimeStatus, ManagedGateway,
};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
struct GatewayStatusResponse {
    id: &'static str,
    title: &'static str,
    enabled: bool,
    task_id: Option<String>,
    status: String,
    pid: Option<u32>,
}

/// 返回网关管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/gateways", get(list))
        .route("/api/gateways/:id/start", post(start))
        .route("/api/gateways/:id/stop", post(stop))
}

/// 查询全部受管理网关状态。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Vec<GatewayStatusResponse>>> {
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let statuses = gateway_runtime_statuses(&state.paths, &config)
        .await
        .map_err(WebError::from)?;
    Ok(Json(statuses.into_iter().map(status_response).collect()))
}

/// 启动指定网关。
async fn start(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let gateway = parse_gateway(&id)?;
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let output = start_gateway(&state.paths, &config, gateway)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let value = serde_json::from_str(&output).unwrap_or_else(|_| json!({ "output": output }));
    Ok(Json(value))
}

/// 停止指定网关。
async fn stop(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Json<Value>> {
    let gateway = parse_gateway(&id)?;
    let config = AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let stopped = stop_gateway(&state.paths, &config, gateway)
        .await
        .map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true, "stopped": stopped })))
}

/// 转换网关运行状态。
fn status_response(status: GatewayRuntimeStatus) -> GatewayStatusResponse {
    GatewayStatusResponse {
        id: status.gateway.id(),
        title: status.gateway.title(),
        enabled: status.enabled,
        task_id: status.task_id,
        status: status.status,
        pid: status.pid,
    }
}

/// 解析受管理网关 ID。
fn parse_gateway(id: &str) -> WebResult<ManagedGateway> {
    match id {
        "qq" => Ok(ManagedGateway::Qq),
        "weixin" => Ok(ManagedGateway::Weixin),
        _ => Err(WebError::not_found(format!("gateway not found: {id}"))),
    }
}
