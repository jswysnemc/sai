use super::super::app_state::WebAppState;
use super::super::browser;
use super::super::error::{WebError, WebResult};
use axum::body::Bytes;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{DefaultBodyLimit, Path, Query};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

/// 上传请求的查询参数。
#[derive(Deserialize)]
struct UploadQuery {
    /// 原文件名，页面读到的文件名与之一致
    name: String,
}

/// 【浏览器面板】【路由】返回内置浏览器面板路由。
/// @returns WebSocket、关闭、清除数据、文件上传与下载接口
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/browser/socket", get(socket))
        .route("/api/browser/close", post(close))
        .route("/api/browser/clear-data", post(clear_data))
        .route(
            "/api/browser/uploads",
            post(upload).layer(DefaultBodyLimit::max(
                crate::browser::MAX_UPLOAD_BYTES + 1024,
            )),
        )
        .route("/api/browser/downloads/:guid", get(download))
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

/// 【浏览器面板】【清除数据】关闭浏览器并删除持久用户目录中的 Cookie、登录状态与缓存。
/// @returns 是否删除了持久目录
async fn clear_data() -> WebResult<Json<Value>> {
    let removed = crate::browser::clear_browsing_data()
        .await
        .map_err(WebError::from)?;
    Ok(Json(json!({ "cleared": removed })))
}

/// 【浏览器面板】【文件上传】保存一个用户选择的文件，返回回复文件选择时使用的 ID。
/// @param query 为原文件名；body 为文件内容
/// @returns 上传 ID
async fn upload(Query(query): Query<UploadQuery>, body: Bytes) -> WebResult<Json<Value>> {
    let id = crate::browser::store_upload(&query.name, &body)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "id": id })))
}

/// 【浏览器面板】【文件下载】把页面下载完成的文件交给用户本机浏览器。
/// @param guid 为下载 GUID
/// @returns 文件内容，附带原文件名
async fn download(Path(guid): Path<String>) -> WebResult<Response> {
    let session = crate::browser::existing()
        .await
        .ok_or_else(|| WebError::not_found("browser is not running"))?;
    let (path, file_name) = session
        .completed_download(&guid)
        .ok_or_else(|| WebError::not_found(format!("download not found: {guid}")))?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| WebError::from(anyhow::Error::from(error)))?;
    let disposition = format!(
        "attachment; filename=\"download\"; filename*=UTF-8''{}",
        urlencoding::encode(&file_name)
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}
