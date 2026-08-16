use crate::web::app_state::WebAppState;
use crate::web::error::{WebError, WebResult};
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path as FilePath, PathBuf};

/// 最近一次真实 HTTP 调试记录。文件内容保持原样，便于导出后与供应商请求逐字比对。
#[derive(Debug, Serialize)]
pub(super) struct LatestDebugResponse {
    pub found: bool,
    pub session_id: String,
    pub request_id: Option<String>,
    pub meta: Option<String>,
    pub request_headers: Option<String>,
    pub request_body: Option<String>,
    pub response_headers: Option<String>,
    pub response_stream: Option<String>,
    pub response_reconstructed: Option<String>,
    pub response_error: Option<String>,
}

/// 一次真实模型请求的最小轨迹快照。
#[derive(Debug, Serialize)]
pub(super) struct DebugRequestSnapshot {
    pub request_id: String,
    pub turn_id: Option<String>,
    pub assistant_round: Option<usize>,
    pub request_body: Option<Value>,
}

/// 返回会话调试导出路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/sessions/:id/debug/latest", axum::routing::get(latest))
        .route(
            "/api/sessions/:id/debug/requests",
            axum::routing::get(requests),
        )
}

/// 读取指定会话最近一次真实 API 请求及响应记录。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 原始请求体、响应流、重组响应及相关元数据
pub(super) async fn latest(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Response> {
    let sessions = crate::state::list_sessions(&state.paths).map_err(WebError::from)?;
    if !sessions.iter().any(|session| session.id == id) {
        return Err(WebError::not_found(format!("session not found: {id}")));
    }

    let root = state.paths.cache_dir.join("debug-http").join(&id);
    let Some(request_dir) = latest_request_dir(&root).map_err(WebError::from)? else {
        return Ok(no_store(empty_response(id)));
    };
    let request_id = request_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    Ok(no_store(LatestDebugResponse {
        found: true,
        session_id: id,
        request_id,
        meta: read_optional(&request_dir, "meta.json").map_err(WebError::from)?,
        request_headers: read_optional(&request_dir, "request_headers.txt")
            .map_err(WebError::from)?,
        request_body: read_optional(&request_dir, "request_body.json").map_err(WebError::from)?,
        response_headers: read_optional(&request_dir, "response_headers.txt")
            .map_err(WebError::from)?,
        response_stream: read_optional(&request_dir, "response_stream.sse")
            .map_err(WebError::from)?,
        response_reconstructed: read_optional(&request_dir, "response_reconstructed.json")
            .map_err(WebError::from)?,
        response_error: read_optional(&request_dir, "response_error.txt")
            .map_err(WebError::from)?,
    }))
}

/// 返回指定会话全部真实模型请求的 system/tools 快照。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `id`: 会话标识
///
/// 返回:
/// - 按实际请求顺序排列的请求体快照；没有调试记录时返回空数组
async fn requests(State(state): State<WebAppState>, Path(id): Path<String>) -> WebResult<Response> {
    let sessions = crate::state::list_sessions(&state.paths).map_err(WebError::from)?;
    if !sessions.iter().any(|session| session.id == id) {
        return Err(WebError::not_found(format!("session not found: {id}")));
    }
    let root = state.paths.cache_dir.join("debug-http").join(&id);
    let mut snapshots = Vec::new();
    for request_dir in request_dirs(&root).map_err(WebError::from)? {
        let request_id = request_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let meta = read_json_optional(&request_dir, "meta.json").map_err(WebError::from)?;
        let turn_id = meta
            .as_ref()
            .and_then(|value| value.get("turn_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let assistant_round = meta
            .as_ref()
            .and_then(|value| value.get("assistant_round"))
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        snapshots.push(DebugRequestSnapshot {
            request_id,
            turn_id,
            assistant_round,
            request_body: read_json_optional(&request_dir, "request_body.json")
                .map_err(WebError::from)?,
        });
    }
    Ok(no_store(snapshots))
}

/// 为包含请求上下文的调试响应添加禁止缓存头。
///
/// 参数:
/// - `payload`: 待序列化的调试响应
///
/// 返回:
/// - 带有 `no-store` 响应头的 HTTP 响应
fn no_store<T: Serialize>(payload: T) -> Response {
    let mut response = Json(payload).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

/// 从记录根目录中选择按时间和序号排序的最新请求目录。
///
/// 参数:
/// - `root`: 会话调试记录根目录
///
/// 返回:
/// - 最新请求目录；目录不存在或为空时返回空值
fn latest_request_dir(root: &FilePath) -> anyhow::Result<Option<PathBuf>> {
    Ok(request_dirs(root)?.pop())
}

/// 读取并按目录名排序所有请求目录。
fn request_dirs(root: &FilePath) -> anyhow::Result<Vec<PathBuf>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = fs::read_dir(root)
        .map_err(anyhow::Error::from)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            Some((entry.file_name().to_str()?.to_string(), entry.path()))
        })
        .collect::<Vec<_>>();
    dirs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(dirs.into_iter().map(|(_, path)| path).collect())
}

/// 读取调试记录文件；单次记录中缺失的文件以空值表示。
///
/// 参数:
/// - `dir`: 单次请求目录
/// - `name`: 文件名
///
/// 返回:
/// - 文件内容；文件不存在时返回空值
fn read_optional(dir: &FilePath, name: &str) -> anyhow::Result<Option<String>> {
    let path = dir.join(name);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path).map_err(anyhow::Error::from)?))
}

/// 读取调试记录中的 JSON 文件。
fn read_json_optional(dir: &FilePath, name: &str) -> anyhow::Result<Option<Value>> {
    let Some(content) = read_optional(dir, name)? else {
        return Ok(None);
    };
    Ok(Some(
        serde_json::from_str(&content).map_err(anyhow::Error::from)?,
    ))
}

/// 构造没有调试记录时的稳定响应结构。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 标记 `found=false` 的响应
fn empty_response(session_id: String) -> LatestDebugResponse {
    LatestDebugResponse {
        found: false,
        session_id,
        request_id: None,
        meta: None,
        request_headers: None,
        request_body: None,
        response_headers: None,
        response_stream: None,
        response_reconstructed: None,
        response_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::latest_request_dir;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn selects_latest_timestamp_and_sequence_directory() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("20260101T000000.000_0001")).unwrap();
        fs::create_dir_all(temp.path().join("20260101T000001.000_0002")).unwrap();
        let latest = latest_request_dir(temp.path()).unwrap().unwrap();
        assert_eq!(
            latest.file_name().unwrap().to_string_lossy(),
            "20260101T000001.000_0002"
        );
    }
}
