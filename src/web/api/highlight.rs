//! 网页代码着色。
//!
//! 浏览器不内置 syntect，着色结果由这里生成。

use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::render::syntect_highlight;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct HighlightRequest {
    source: String,
    language: Option<String>,
}

#[derive(Serialize)]
struct HighlightResponse {
    html: String,
    language: String,
}

/// 返回代码着色路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/highlight", post(highlight))
}

/// 用 syntect 把源码着色成 HTML。
///
/// 参数:
/// - `request`: 源码和可选语言
///
/// 返回:
/// - 安全 HTML 与规范化语言
async fn highlight(Json(request): Json<HighlightRequest>) -> WebResult<Json<HighlightResponse>> {
    if request.source.chars().count() > 200_000 {
        return Err(WebError::bad_request("source is too large"));
    }
    let language = request.language.unwrap_or_default();
    // 1. 解析发生在请求线程之外，避免挡住其他连接
    let source = request.source;
    let (html, normalized) =
        tokio::task::spawn_blocking(move || syntect_highlight::highlight_html(&source, &language))
            .await
            .map_err(|_| WebError::bad_request("highlight failed"))?;
    Ok(Json(HighlightResponse {
        html,
        language: normalized,
    }))
}
