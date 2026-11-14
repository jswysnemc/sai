use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

mod cache_policy;
mod current_pointer;
pub(super) mod distribution;
mod installation;

#[derive(Clone)]
pub(super) struct WebAssets(Arc<PathBuf>);

#[cfg(test)]
mod distribution_tests;
#[cfg(test)]
mod tests;

impl WebAssets {
    /// 【Web 资源】【目录绑定】固定服务使用的资源版本，更新不会改变正在运行的实例
    /// 参数: directory 为资源目录；返回: 已验证的资源句柄或错误
    pub(super) fn new(directory: PathBuf) -> anyhow::Result<Self> {
        let directory = directory.canonicalize()?;
        anyhow::ensure!(
            directory.join("index.html").is_file(),
            "Web index.html is missing"
        );
        Ok(Self(Arc::new(directory)))
    }

    /// 【Web 资源】【文件读取】读取根目录内文件，阻止路径穿越及指向外部的符号链接
    /// 参数: path 为相对路径；返回: 文件内容，缺失或非法路径返回空
    async fn read(&self, path: &str) -> Option<Vec<u8>> {
        if !safe_relative_path(path) {
            return None;
        }
        let file = tokio::fs::canonicalize(self.0.join(path)).await.ok()?;
        if !file.starts_with(self.0.as_ref()) {
            return None;
        }
        tokio::fs::read(file).await.ok()
    }
}

/// 【Web 资源】【路径校验】拒绝绝对路径、父目录及跨平台特殊分隔符
/// 参数: path 为解码后的路径；返回: 是否安全
fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

/// 【Web 资源】【HTTP 响应】提供本地资源，页面路由回退到 index.html。
///
/// 参数:
/// - `assets`: 当前服务固定使用的资源目录
/// - `uri`: 请求 URI
/// - `method`: 请求方法
/// - `headers`: 压缩协商与条件请求头
///
/// 返回:
/// - 静态资源响应
pub(super) async fn serve(
    Extension(assets): Extension<WebAssets>,
    uri: Uri,
    method: Method,
    headers: HeaderMap,
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Ok(decoded) = urlencoding::decode(uri.path().trim_start_matches('/')) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let path = if decoded.is_empty() {
        "index.html"
    } else {
        decoded.as_ref()
    };
    if !safe_relative_path(path) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let (asset_path, data) = match assets.read(path).await {
        Some(data) => (path, Some(data)),
        None if Path::new(path).extension().is_none() => {
            ("index.html", assets.read("index.html").await)
        }
        None => (path, None),
    };
    let Some(data) = data else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // 1. 【静态资源】【缓存校验】按原始内容生成弱标签，供压缩与原始表示共同校验
    let etag = format!("W/\"{}\"", hex::encode(Sha256::digest(&data)));
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
    let compressed = if cache_policy::accepts_gzip(&headers) {
        assets.read(&format!("{asset_path}.gz")).await
    } else {
        None
    };
    let data = if let Some(compressed) = compressed {
        response_headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        compressed
    } else {
        data
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
