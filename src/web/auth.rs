use super::app_state::WebAppState;
use super::error::{WebError, WebResult};
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

const SESSION_COOKIE: &str = "sai_web_session";

#[derive(Deserialize)]
pub(super) struct SessionQuery {
    token: String,
}

#[derive(Serialize)]
struct SessionResponse {
    ok: bool,
}

/// 使用启动令牌建立浏览器会话 Cookie。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `query`: 启动令牌
///
/// 返回:
/// - 设置安全 Cookie 的响应
pub(super) async fn create_session(
    State(state): State<WebAppState>,
    Query(query): Query<SessionQuery>,
) -> WebResult<Response> {
    if query.token.as_bytes() != state.auth_token.as_bytes() {
        return Err(WebError::unauthorized());
    }
    let cookie = format!(
        "{SESSION_COOKIE}={}; Path=/; HttpOnly; SameSite=Strict",
        state.auth_token
    );
    let mut response = (StatusCode::OK, Json(SessionResponse { ok: true })).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|error| WebError::bad_request(error.to_string()))?,
    );
    Ok(response)
}

/// 校验受保护 API 的 Cookie 或 Bearer 令牌。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 当前 HTTP 请求
/// - `next`: 下一个处理器
///
/// 返回:
/// - 下游响应或未授权响应
pub(super) async fn require_auth(
    State(state): State<WebAppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request_token(&request).as_deref() == Some(state.auth_token.as_ref()) {
        return next.run(request).await;
    }
    WebError::unauthorized().into_response()
}

/// 从请求 Cookie 或 Authorization 读取令牌。
fn request_token(request: &Request<Body>) -> Option<String> {
    let bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string);
    if bearer.is_some() {
        return bearer;
    }
    request
        .headers()
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == SESSION_COOKIE).then(|| value.to_string())
            })
        })
}
