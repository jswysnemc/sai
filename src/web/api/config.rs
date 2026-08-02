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

/// 当前对话内核状态。
#[derive(Serialize)]
struct EngineStatusResponse {
    /// 内核标识
    engine: String,
    /// 展示名称
    label: String,
    /// 是否为外部 ACP 内核
    external: bool,
    /// 该内核下不可用的 sai 功能
    unavailable_features: Vec<String>,
    /// 最近一次 ACP 握手能力；尚未启动外部内核时为空
    acp_capabilities: Option<crate::acp::AcpCapabilities>,
    /// 最近一次 ACP 握手及会话配置快照
    acp_runtime: Option<crate::acp::AcpRuntimeState>,
}

/// 返回配置管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/config", get(load).put(save))
        .route("/api/config/provider-secret", post(provider_secret))
        .route("/api/config/rtk-status", get(rtk_status))
        .route("/api/config/engine-status", get(engine_status))
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

/// 返回当前对话内核状态。
///
/// 前端据此标注模型与上下文这些在外部内核下不生效的信息，
/// 避免用户以为自己选的模型还在起作用。
async fn engine_status(State(state): State<WebAppState>) -> WebResult<Json<EngineStatusResponse>> {
    let config = config_service::load_redacted(&state.paths).map_err(WebError::from)?;
    let engine = serde_json::from_value::<crate::config::AgentEngineConfig>(
        config.get("agent").cloned().unwrap_or(Value::Null),
    )
    .unwrap_or_default()
    .engine;
    Ok(Json(EngineStatusResponse {
        engine: engine.as_str().to_string(),
        label: engine.display_label().to_string(),
        external: engine.is_external(),
        unavailable_features: engine
            .unavailable_features()
            .into_iter()
            .map(str::to_string)
            .collect(),
        acp_capabilities: crate::acp::current_capabilities(engine.as_str()),
        acp_runtime: crate::acp::current_runtime_state(engine.as_str()),
    }))
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
