use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::header::CACHE_CONTROL;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use sai_plugin_runtime::{PresentationSurface, ReplyPresentation, ReplyStatus};
use serde::Deserialize;

/// 【通知接口】【请求】只接收展示所需状态与语言，不接受插件授权或设置覆盖。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotificationRequest {
    status: ReplyStatus,
    locale: String,
}

/// 【通知接口】【路由】注册需要现有 Web 身份验证的纯策略接口。
/// @returns 仅计算通知数据的路由，不向服务端桌面投递通知
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route(
        "/api/notifications/plan",
        post(plan).layer(DefaultBodyLimit::max(1024)),
    )
}

/// 【通知接口】【策略计算】读取最新插件配置并返回浏览器可投递的数据。
/// @param state Web 应用状态；request 为有限的通知事件
/// @returns 禁止缓存的通知计划；策略失败以独立诊断返回
async fn plan(
    State(state): State<WebAppState>,
    Json(request): Json<NotificationRequest>,
) -> WebResult<Response> {
    let event = ReplyPresentation {
        surface: PresentationSurface::Web,
        status: request.status,
        locale: request.locale,
    };
    event
        .validate()
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let config = AppConfig::load_or_default(&state.paths)?;
    let plan = crate::plugins::notification_plan(&config, &state.paths, event).await?;
    Ok(([(CACHE_CONTROL, "no-store")], Json(plan)).into_response())
}
