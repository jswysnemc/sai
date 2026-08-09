use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::ssh::{import_candidates, trust_host_key, HostKey, KnownHostStatus};
use crate::config::{AppConfig, SshHostConfig, DEFAULT_SSH_PORT};
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct SaveHostRequest {
    label: String,
    hostname: String,
    port: Option<u16>,
    username: String,
    identity_file: Option<String>,
    remote_directory: Option<String>,
}

#[derive(Deserialize)]
struct ImportHostsRequest {
    hosts: Vec<SaveHostRequest>,
}

#[derive(Deserialize)]
struct TrustHostKeyRequest {
    hostname: String,
    port: u16,
    algorithm: String,
    key_base64: String,
    fingerprint: String,
}

/// 返回 SSH 主机管理路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/ssh/hosts", get(list).post(create))
        .route("/api/ssh/hosts/:id", axum::routing::put(update).delete(remove))
        .route("/api/ssh/hosts/import", get(scan_config).post(import))
        .route("/api/ssh/known-hosts/trust", post(trust))
}

/// 校验并规整主机表单。
///
/// 参数:
/// - `request`: 表单内容
/// - `id`: 主机标识
///
/// 返回:
/// - 规整后的主机配置
fn build_host(request: SaveHostRequest, id: String) -> WebResult<SshHostConfig> {
    let hostname = request.hostname.trim().to_string();
    if hostname.is_empty() {
        return Err(WebError::bad_request("hostname is required"));
    }
    let username = request.username.trim().to_string();
    if username.is_empty() {
        return Err(WebError::bad_request("username is required"));
    }
    let port = request.port.unwrap_or(DEFAULT_SSH_PORT);
    if port == 0 {
        return Err(WebError::bad_request("port must be between 1 and 65535"));
    }
    // 标签留空时回落为主机名，列表始终有可读名称
    let label = match request.label.trim() {
        "" => hostname.clone(),
        value => value.to_string(),
    };
    Ok(SshHostConfig {
        id,
        label,
        hostname,
        port,
        username,
        identity_file: request.identity_file.unwrap_or_default().trim().to_string(),
        remote_directory: request
            .remote_directory
            .unwrap_or_default()
            .trim()
            .to_string(),
    })
}

/// 读取当前配置。
fn load_config(state: &WebAppState) -> WebResult<AppConfig> {
    AppConfig::load_or_default(&state.paths).map_err(WebError::from)
}

/// 列出已配置的 SSH 主机。
async fn list(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    Ok(Json(json!({ "hosts": config.ssh.hosts })))
}

/// 新增 SSH 主机。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<SaveHostRequest>,
) -> WebResult<Json<Value>> {
    let mut config = load_config(&state)?;
    let host = build_host(request, format!("ssh_{}", uuid::Uuid::new_v4().simple()))?;
    config.ssh.hosts.push(host.clone());
    config.save(&state.paths).map_err(WebError::from)?;
    Ok(Json(json!({ "host": host })))
}

/// 更新指定 SSH 主机。
async fn update(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<SaveHostRequest>,
) -> WebResult<Json<Value>> {
    let mut config = load_config(&state)?;
    let index = config
        .ssh
        .hosts
        .iter()
        .position(|host| host.id == id)
        .ok_or_else(|| WebError::not_found(format!("ssh host not found: {id}")))?;
    let host = build_host(request, id)?;
    config.ssh.hosts[index] = host.clone();
    config.save(&state.paths).map_err(WebError::from)?;
    Ok(Json(json!({ "host": host })))
}

/// 删除指定 SSH 主机。
async fn remove(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<Value>> {
    let mut config = load_config(&state)?;
    let before = config.ssh.hosts.len();
    config.ssh.hosts.retain(|host| host.id != id);
    let removed = config.ssh.hosts.len() != before;
    if removed {
        config.save(&state.paths).map_err(WebError::from)?;
    }
    Ok(Json(json!({ "removed": removed })))
}

/// 扫描 `~/.ssh/config` 并返回可导入的主机。
async fn scan_config(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    let config = load_config(&state)?;
    let candidates = import_candidates(&config.ssh.hosts).map_err(WebError::from)?;
    Ok(Json(json!({ "candidates": candidates })))
}

/// 批量导入选中的主机。
async fn import(
    State(state): State<WebAppState>,
    Json(request): Json<ImportHostsRequest>,
) -> WebResult<Json<Value>> {
    let mut config = load_config(&state)?;
    let mut imported = Vec::new();
    for host in request.hosts {
        let host = build_host(host, format!("ssh_{}", uuid::Uuid::new_v4().simple()))?;
        imported.push(host.clone());
        config.ssh.hosts.push(host);
    }
    config.save(&state.paths).map_err(WebError::from)?;
    Ok(Json(json!({ "hosts": imported })))
}

/// 把用户确认过的主机密钥写入 known_hosts。
async fn trust(Json(request): Json<TrustHostKeyRequest>) -> WebResult<Json<Value>> {
    let key = HostKey {
        hostname: request.hostname,
        port: request.port,
        algorithm: request.algorithm,
        key_base64: request.key_base64,
        fingerprint: request.fingerprint,
    };
    trust_host_key(&key).map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(json!({ "trusted": true })))
}

/// 把主机密钥校验结论转换为可返回给前端的结构。
///
/// 参数:
/// - `key`: 远端主机密钥
/// - `status`: 校验结论
///
/// 返回:
/// - 供前端展示指纹与告警的 JSON
pub(super) fn host_key_prompt(key: &HostKey, status: &KnownHostStatus) -> Value {
    json!({
        "hostname": key.hostname,
        "port": key.port,
        "algorithm": key.algorithm,
        "key_base64": key.key_base64,
        "fingerprint": key.fingerprint,
        "changed": matches!(status, KnownHostStatus::Changed { .. }),
    })
}
