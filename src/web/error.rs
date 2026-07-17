use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Web API 统一错误。
#[derive(Debug)]
pub(super) struct WebError {
    status: StatusCode,
    message: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl WebError {
    /// 创建指定状态码的 API 错误。
    ///
    /// 参数:
    /// - `status`: HTTP 状态码
    /// - `message`: 错误文本
    ///
    /// 返回:
    /// - Web API 错误
    pub(super) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// 创建请求参数错误。
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// 创建资源冲突错误。
    pub(super) fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    /// 创建资源不存在错误。
    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    /// 创建未授权错误。
    pub(super) fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }
}

impl IntoResponse for WebError {
    /// 将错误转换为 JSON HTTP 响应。
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for WebError {
    /// 将内部错误转换为服务错误。
    fn from(error: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

pub(super) type WebResult<T> = Result<T, WebError>;
