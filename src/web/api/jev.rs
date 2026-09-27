use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::{jev_connection_for, JevConnectionInfo, ModelEndpointConfig, ModelEndpointKind};
use crate::jev::probe::{self, ProbeReport};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// 连接测试请求；省略接入时测试当前保存的生效接入。
#[derive(Debug, Deserialize)]
struct TestRequest {
    #[serde(default)]
    endpoint: Option<ModelEndpointConfig>,
}

/// 当前 Jev 功能状态。
#[derive(Debug, Serialize)]
struct StatusResponse {
    /// 生效接入；`endpoint_id` 指向不存在的接入时为空
    connection: Option<JevConnectionInfo>,
    /// 密钥能否解析
    key_ready: bool,
    /// 接入或密钥不可用的原因
    error: Option<String>,
    /// 暴露决策是否开启
    routing_enabled: bool,
    /// 暴露决策开启但被 DeepSeek 锚定模式接管时为 false
    routing_active: bool,
    /// 内置审核是否开启
    audit_enabled: bool,
    /// 自动审核实际使用的后端：jev、plugin 或 model
    audit_backend: &'static str,
}

/// 返回 Jev 状态与连接测试路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/jev/status", get(status))
        .route("/api/jev/test", post(test))
}

/// 【Jev接入】【状态查询】读取已保存配置，报告接入、密钥与两项功能的生效情况。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 不含密钥的状态
async fn status(State(state): State<WebAppState>) -> WebResult<Json<StatusResponse>> {
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    // 1. 接入与密钥分开判断，便于界面区分“接入缺失”和“密钥缺失”
    let (connection, error) = match config.jev_endpoint() {
        Ok(endpoint) => match jev_connection_for(endpoint) {
            Ok(connection) => (Some(connection.info), None),
            Err(error) => (
                Some(crate::config::jev_connection_info_for(endpoint)),
                Some(format!("{error:#}")),
            ),
        },
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    // 2. 审核后端与运行时 AutoAuditBackend::resolve 的优先级保持一致
    let audit_backend = if config.jev.audit.enabled {
        "jev"
    } else if !config.permission.auto_audit_plugin_id.trim().is_empty() {
        "plugin"
    } else {
        "model"
    };
    Ok(Json(StatusResponse {
        key_ready: error.is_none(),
        connection,
        error,
        routing_enabled: config.jev.routing.enabled,
        routing_active: config.jev_routing_active(),
        audit_enabled: config.jev.audit.enabled,
        audit_backend,
    }))
}

/// 【Jev接入】【连接测试】对草稿接入或当前生效接入发送一次最小请求。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 可选的接入草稿
///
/// 返回:
/// - 测试结果；接入无法解析时同样以失败结果返回
async fn test(
    State(state): State<WebAppState>,
    Json(request): Json<TestRequest>,
) -> WebResult<Json<ProbeReport>> {
    // 1. 草稿接入先还原脱敏密钥；未提供时读取已保存配置
    let connection = match request.endpoint {
        Some(draft) => {
            let endpoint = super::model_endpoint_draft::restore_endpoint_secrets(
                &state,
                draft,
                ModelEndpointKind::Jev,
            )?;
            jev_connection_for(Some(&endpoint))
        }
        None => crate::config::AppConfig::load_or_default(&state.paths)
            .map_err(WebError::from)?
            .jev_connection(),
    };
    // 2. 接入不可用时直接返回失败原因
    let connection = match connection {
        Ok(connection) => connection,
        Err(error) => return Err(WebError::bad_request(format!("{error:#}"))),
    };
    Ok(Json(probe::probe(&connection).await))
}
