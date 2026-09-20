use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// 返回已完成生图的缓存文件。
///
/// 参数:
/// - state: Web 应用状态
/// - file_name: 仅允许缓存目录中的单层文件名
///
/// 返回:
/// - 带真实图片媒体类型的响应
pub(super) async fn image(
    State(state): State<WebAppState>,
    Path(file_name): Path<String>,
) -> WebResult<Response> {
    if !is_safe_file_name(&file_name) {
        return Err(WebError::bad_request("invalid generated image name"));
    }
    let path = state
        .paths
        .cache_dir
        .join("generated-images")
        .join(&file_name);
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| WebError::not_found(error.to_string()))?;
    if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES {
        return Err(WebError::not_found("generated image is unavailable"));
    }
    let mime = mime_guess::from_path(&path)
        .first()
        .filter(|mime| mime.type_() == mime_guess::mime::IMAGE)
        .ok_or_else(|| WebError::bad_request("generated file is not an image"))?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    let content_type = HeaderValue::from_str(mime.as_ref())
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    let content_length = HeaderValue::from_str(&bytes.len().to_string())
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)?;
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, content_length)
        .body(axum::body::Body::from(bytes))
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)
}

/// 返回生成图片缓存路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new().route("/api/generated-images/:file_name", get(image))
}

/// 校验媒体文件名不含目录分隔符或隐藏路径片段。
fn is_safe_file_name(file_name: &str) -> bool {
    !file_name.is_empty()
        && file_name != "."
        && file_name != ".."
        && file_name.len() <= 160
        && file_name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && file_name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::is_safe_file_name;

    #[test]
    fn accepts_generated_names_and_rejects_paths() {
        assert!(is_safe_file_name("abc-1.png"));
        assert!(!is_safe_file_name("../abc.png"));
        assert!(!is_safe_file_name("a/b.png"));
    }
}
