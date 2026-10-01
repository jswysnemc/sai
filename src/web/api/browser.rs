use super::super::app_state::WebAppState;
use super::super::browser;
use super::super::error::WebResult;
use axum::extract::ws::WebSocketUpgrade;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

/// 【浏览器面板】【路由】返回内置浏览器面板路由。
/// @returns 浏览器面板 WebSocket 与关闭接口
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/browser/socket", get(socket))
        .route("/api/browser/close", post(close))
}

/// 【浏览器面板】【连接升级】升级为浏览器面板 WebSocket。
/// @param upgrade 为 WebSocket 升级请求
/// @returns 升级响应
async fn socket(upgrade: WebSocketUpgrade) -> WebResult<Response> {
    Ok(upgrade
        .max_message_size(1 << 20)
        .on_upgrade(browser::serve_socket))
}

/// 【浏览器面板】【关闭浏览器】结束共享浏览器进程，下次使用时重新启动。
/// @returns 是否存在被关闭的会话
async fn close() -> WebResult<Json<Value>> {
    let existed = crate::browser::existing().await.is_some();
    crate::browser::shutdown().await;
    Ok(Json(json!({ "closed": existed })))
}
