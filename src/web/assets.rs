use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

mod cache_policy;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

#[cfg(test)]
mod tests;

/// 返回内嵌前端资源，未知前端路由回退到 index.html。
///
/// 参数:
/// - `uri`: 请求 URI
/// - `method`: 请求方法
/// - `headers`: 压缩协商与条件请求头
///
/// 返回:
/// - 静态资源响应
pub(super) async fn serve(uri: Uri, method: Method, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let (asset_path, asset) = match WebAssets::get(path) {
        Some(asset) => (path, Some(asset)),
        None => ("index.html", WebAssets::get("index.html")),
    };
    let Some(asset) = asset else {
        return (StatusCode::NOT_FOUND, "web assets are not built").into_response();
    };
    // 1. 【静态资源】【缓存校验】按原始内容生成弱标签，供压缩与原始表示共同校验
    let etag = format!("W/\"{}\"", hex::encode(asset.metadata.sha256_hash()));
    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_policy::cache_control(asset_path)),
    );
    response_headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if (method == Method::GET || method == Method::HEAD)
        && cache_policy::is_not_modified(&headers, &etag)
    {
        return (StatusCode::NOT_MODIFIED, response_headers).into_response();
    }

    // 2. 【静态资源】【压缩交付】直接选择构建期生成的 gzip 文件，缺少压缩版本时使用原始内容
    let compressed = cache_policy::accepts_gzip(&headers)
        .then(|| WebAssets::get(&format!("{asset_path}.gz")))
        .flatten();
    let data = if let Some(compressed) = compressed {
        response_headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        compressed.data
    } else {
        asset.data
    };
    let mime = mime_guess::from_path(asset_path).first_or_octet_stream();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref()).unwrap(),
    );
    response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(data.len()));
    if method == Method::HEAD {
        return (StatusCode::OK, response_headers, Body::empty()).into_response();
    }
    (StatusCode::OK, response_headers, data).into_response()
}
