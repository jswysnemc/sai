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

#[derive(Deserialize)]
pub(super) struct PasswordLoginRequest {
    password: String,
}

#[derive(Serialize)]
struct SessionResponse {
    ok: bool,
}

#[derive(Serialize)]
pub(super) struct AuthModeResponse {
    /// 为真时浏览器需先通过口令登录
    password_required: bool,
}

/// 返回当前实例的认证方式。
///
/// 该端点不受保护：登录页需要在持有凭据之前判断该显示口令表单还是直接跳转。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 是否启用口令验证
pub(super) async fn auth_mode(State(state): State<WebAppState>) -> Json<AuthModeResponse> {
    Json(AuthModeResponse {
        password_required: state.password_hash.is_some(),
    })
}

/// 使用访问口令建立浏览器会话 Cookie。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 待校验口令
///
/// 返回:
/// - 设置安全 Cookie 的响应
pub(super) async fn password_login(
    State(state): State<WebAppState>,
    Json(request): Json<PasswordLoginRequest>,
) -> WebResult<Response> {
    let Some(stored) = state.password_hash.as_deref() else {
        return Err(WebError::bad_request("password login is not enabled"));
    };
    let matched = super::password::verify_web_password(&request.password, stored)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    if !matched {
        // 口令错误按固定延迟返回，压制在线枚举速率
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        return Err(WebError::unauthorized());
    }
    session_response(&state)
}

/// 使用启动令牌建立浏览器会话 Cookie。
///
/// 启用口令验证后令牌不再单独放行：对外监听时令牌会出现在启动日志与命令历史里，
/// 只有同时通过口令才能建立会话。
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
    if state.password_hash.is_some() {
        return Err(WebError::unauthorized());
    }
    if query.token.as_bytes() != state.auth_token.as_bytes() {
        return Err(WebError::unauthorized());
    }
    session_response(&state)
}

/// 组装携带会话 Cookie 的成功响应。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 设置安全 Cookie 的响应
fn session_response(state: &WebAppState) -> WebResult<Response> {
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
