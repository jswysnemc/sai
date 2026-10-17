use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspace;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path as FsPath;

#[derive(Deserialize)]
struct PreviewRequest {
    path: String,
}

/**
 * 【工作区】【HTML 预览】签发预览路径。调用方已通过登录校验。
 * @returns 受保护的创建接口
 */
pub(super) fn protected_routes() -> Router<WebAppState> {
    Router::new().route("/api/workspace/html-preview", post(create_preview))
}

/**
 * 【工作区】【HTML 预览】供内置浏览器直接读取页面和相对资源。
 *
 * 内置浏览器是另一个进程，不会带上 Sai 的登录 Cookie，因此这条路由放在登录校验之外，
 * 只认创建时签发的令牌。
 * @returns 公开读取接口
 */
pub(super) fn public_routes() -> Router<WebAppState> {
    Router::new().route("/api/html-preview/:token/*path", get(serve_preview))
}

/**
 * 【工作区】【HTML 预览】确认文件是 HTML 后返回预览路径。
 * @param state 为应用状态
 * @param request 为目标文件路径
 * @returns 预览路径
 */
async fn create_preview(
    State(state): State<WebAppState>,
    Json(request): Json<PreviewRequest>,
) -> WebResult<Json<Value>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let path = workspace::html_preview_url(FsPath::new(&active.path), &request.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "path": path })))
}

/**
 * 【工作区】【HTML 预览】按令牌返回文件内容。
 * @param token 为预览授权
 * @param path 为授权根下的相对路径
 * @returns 带媒体类型的文件响应
 */
async fn serve_preview(Path((token, path)): Path<(String, String)>) -> WebResult<Response> {
    let (bytes, mime) = workspace::read_html_preview(&token, &path)
        .map_err(|_| WebError::not_found("preview file is unavailable"))?;
    let content_type = HeaderValue::from_str(mime)
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    let content_length = HeaderValue::from_str(&bytes.len().to_string())
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, content_length)
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(axum::body::Body::from(bytes))
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)
}
