use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Serialize;

/// 预热连接结果。
#[derive(Serialize)]
struct EngineConnectResponse {
    /// agent 自报名称
    agent: String,
    /// agent 自报版本
    version: String,
}

/// 返回外部内核连接管理路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 连接与断开路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/config/engine-connect", post(engine_connect))
        .route("/api/config/engine-disconnect", post(engine_disconnect))
}

/// 【ACP】【手动连接】主动连接外部内核并抓取其运行时能力。
///
/// 外部内核默认延迟启动，首轮对话之前界面拿不到可选模型与思考等级。
/// 本端点完成一次握手与建会话，把能力写入运行状态后回收进程。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 握手成功时返回 agent 身份，失败时返回可展示的错误
async fn engine_connect(
    State(state): State<WebAppState>,
) -> WebResult<Json<EngineConnectResponse>> {
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let outcome = crate::acp::warm_up(&config, &state.paths)
        .await
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(EngineConnectResponse {
        agent: outcome.agent,
        version: outcome.version,
    }))
}

/// 【ACP】【手动连接】清除已记录的外部内核运行状态。
///
/// 预热握手结束后子进程已经回收，这里只需丢弃缓存的能力快照，
/// 让界面回到未连接展示并允许下一次重新握手。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 始终成功
async fn engine_disconnect(State(state): State<WebAppState>) -> WebResult<Json<serde_json::Value>> {
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    crate::acp::clear_runtime_state(config.agent.engine.as_str());
    Ok(Json(serde_json::json!({ "cleared": true })))
}
