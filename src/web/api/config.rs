use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::{self, SECRET_SENTINEL};
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize)]
struct ConfigResponse {
    config: Value,
    secret_sentinel: &'static str,
}

#[derive(Deserialize)]
struct ProviderSecretRequest {
    provider_id: String,
    /// 多密钥场景下指定要查看的密钥标识；缺省返回单值密钥
    key_id: Option<String>,
}

#[derive(Deserialize)]
struct ModelEndpointSecretRequest {
    endpoint_id: String,
    key_id: Option<String>,
}

#[derive(Serialize)]
struct ProviderSecretResponse {
    api_key: String,
}

#[derive(Serialize)]
struct RtkStatusResponse {
    available: bool,
    /// rtk 当前版本可代理的命令，运行时从 `rtk --help` 探测
    proxy_commands: Vec<String>,
}

/// 返回配置管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/config", get(load).put(save))
        .route("/api/config/provider-secret", post(provider_secret))
        .route(
            "/api/config/model-endpoint-secret",
            post(model_endpoint_secret),
        )
        .route("/api/config/rtk-status", get(rtk_status))
        .route("/api/config/sandbox-status", get(sandbox_status))
}

/// 按需读取供应商真实 API Key。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 供应商稳定标识
///
/// 返回:
/// - 禁止缓存的真实 API Key 响应
async fn provider_secret(
    State(state): State<WebAppState>,
    Json(request): Json<ProviderSecretRequest>,
) -> WebResult<Response> {
    let api_key = config_service::load_provider_secret(
        &state.paths,
        &request.provider_id,
        request.key_id.as_deref(),
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    let mut response = Json(ProviderSecretResponse { api_key }).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    Ok(response)
}

/// 按需读取专用模型端点的真实 API Key。
async fn model_endpoint_secret(
    State(state): State<WebAppState>,
    Json(request): Json<ModelEndpointSecretRequest>,
) -> WebResult<Response> {
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let endpoint = config
        .model_endpoints
        .iter()
        .find(|endpoint| endpoint.id == request.endpoint_id)
        .ok_or_else(|| WebError::not_found("model endpoint not found"))?;
    let api_key = endpoint
        .resolved_api_key_for_key(request.key_id.as_deref())
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let mut response = Json(ProviderSecretResponse { api_key }).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    Ok(response)
}

/// 返回 rtk 可用状态与可代理命令集合，供设置页展示。
async fn rtk_status() -> Json<RtkStatusResponse> {
    Json(RtkStatusResponse {
        available: crate::tools::command::rtk_available(),
        proxy_commands: crate::tools::command::rtk_proxy_commands()
            .iter()
            .cloned()
            .collect(),
    })
}

/// 【Web 设置】【沙箱状态】返回后端探测结果与当前工作区解析出的沙箱路径。
///
/// 参数:
/// - `state`: Web 应用状态，用于先按磁盘配置刷新快照
///
/// 返回:
/// - 沙箱状态
async fn sandbox_status(
    State(state): State<WebAppState>,
) -> WebResult<Json<crate::sandbox::SandboxStatus>> {
    // 1. 状态页可能早于任何运行打开，先按磁盘配置安装快照
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    crate::sandbox::install(&config.sandbox, &state.paths);
    // 2. 探测会同步启动一次空命令，放到阻塞线程执行
    let status = tokio::task::spawn_blocking(crate::sandbox::sandbox_status)
        .await
        .map_err(|error| WebError::from(anyhow::anyhow!(error)))?;
    Ok(Json(status))
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
