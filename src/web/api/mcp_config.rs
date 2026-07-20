use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::{self, SECRET_SENTINEL};
use crate::config::{load_mcp_config, save_mcp_config, McpConfig, McpServerConfig};
use crate::mcp::{list_server_tools, McpToolInfo};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
struct McpConfigResponse {
    config: Value,
    path: String,
    secret_sentinel: &'static str,
}

/// 返回独立 MCP 配置路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/config/mcp", get(load).put(save))
        .route("/api/config/mcp/tools", axum::routing::post(scan_tools))
}

/// 读取脱敏后的 MCP 配置。
async fn load(State(state): State<WebAppState>) -> WebResult<Json<McpConfigResponse>> {
    let config = load_mcp_config(&state.paths).map_err(WebError::from)?;
    let mut value = serde_json::to_value(config)
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    config_service::redact_json_value(&mut value);
    Ok(Json(McpConfigResponse {
        config: value,
        path: state.paths.mcp_config_file().display().to_string(),
        secret_sentinel: SECRET_SENTINEL,
    }))
}

/// 校验并保存独立 MCP 配置。
async fn save(
    State(state): State<WebAppState>,
    Json(submitted): Json<Value>,
) -> WebResult<Json<McpConfigResponse>> {
    let current = serde_json::to_value(load_mcp_config(&state.paths).map_err(WebError::from)?)
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    let mut submitted = submitted;
    config_service::merge_secret_sentinels_json(&mut submitted, &current);
    let config: McpConfig = serde_json::from_value(submitted)
        .map_err(|error| WebError::bad_request(format!("invalid MCP configuration: {error}")))?;
    save_mcp_config(&state.paths, &config)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    load(State(state)).await
}

#[derive(Serialize)]
struct McpToolsResponse {
    tools: Vec<McpToolInfo>,
}

/// 使用当前表单中的服务配置扫描 MCP 工具详情。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `submitted`: 待扫描的单个 MCP 服务配置
///
/// 返回:
/// - 工具名称、说明与输入参数结构
async fn scan_tools(
    State(state): State<WebAppState>,
    Json(submitted): Json<Value>,
) -> WebResult<Json<McpToolsResponse>> {
    let current = load_mcp_config(&state.paths).map_err(WebError::from)?;
    let current_server = submitted
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| current.servers.iter().find(|server| server.id == id));
    let mut submitted = submitted;
    if let Some(server) = current_server {
        let current_value = serde_json::to_value(server)
            .map_err(anyhow::Error::from)
            .map_err(WebError::from)?;
        config_service::merge_secret_sentinels_json(&mut submitted, &current_value);
    }
    let server: McpServerConfig = serde_json::from_value(submitted).map_err(|error| {
        WebError::bad_request(format!("invalid MCP server configuration: {error}"))
    })?;
    let tools = list_server_tools(&server)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(McpToolsResponse { tools }))
}
