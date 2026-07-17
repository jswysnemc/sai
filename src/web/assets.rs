use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

/// 返回内嵌前端资源，未知前端路由回退到 index.html。
///
/// 参数:
/// - `uri`: 请求 URI
///
/// 返回:
/// - 静态资源响应
pub(super) async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let (asset_path, asset) = match WebAssets::get(path) {
        Some(asset) => (path, Some(asset)),
        None => ("index.html", WebAssets::get("index.html")),
    };
    let Some(asset) = asset else {
        return (StatusCode::NOT_FOUND, "web assets are not built").into_response();
    };
    let mime = mime_guess::from_path(asset_path).first_or_octet_stream();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime.as_ref())],
        asset.data,
    )
        .into_response()
}
