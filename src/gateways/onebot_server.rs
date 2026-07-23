use super::message::OutboundMessage;
use super::onebot::OneBotClient;
use super::onebot_event::{
    parse_message_event, OneBotInboundMedia, OneBotInboundMediaKind, OneBotMessageEvent,
};
use crate::agent::AgentMode;
use crate::gateways::command_intercept::handle_gateway_command;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const MAX_INBOUND_MEDIA_BYTES: usize = 20 * 1024 * 1024;

pub(crate) struct OneBotServerConfig {
    pub(crate) listen: SocketAddr,
    pub(crate) onebot_base_url: String,
    pub(crate) access_token: Option<String>,
}

struct OneBotInboundState {
    paths: SaiPaths,
    onebot_base_url: String,
    access_token: Option<String>,
    http_client: reqwest::Client,
    agent_lock: Mutex<()>,
}

/// 启动 OneBot 入站 HTTP 服务。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: OneBot 入站服务配置
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run_onebot_server(paths: &SaiPaths, config: OneBotServerConfig) -> Result<()> {
    validate_server_config(&config)?;
    let listen = config.listen;
    let state = Arc::new(OneBotInboundState {
        paths: paths.clone(),
        onebot_base_url: config.onebot_base_url,
        access_token: config.access_token,
        http_client: reqwest::Client::new(),
        agent_lock: Mutex::new(()),
    });
    let app = Router::new()
        .route("/", post(handle_onebot_event))
        .route("/onebot", post(handle_onebot_event))
        .with_state(state);
    let listener = TcpListener::bind(listen).await.with_context(|| {
        format!(
            "{}: {listen}",
            t(
                "failed to bind OneBot inbound server",
                "无法绑定 OneBot 入站服务"
            )
        )
    })?;
    println!(
        "{} http://{listen}",
        t(
            "OneBot inbound server listening on",
            "OneBot 入站服务监听地址"
        )
    );
    axum::serve(listener, app).await?;
    Ok(())
}

/// 接收 OneBot 上报事件。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `payload`: OneBot 上报事件 JSON
///
/// 返回:
/// - OneBot HTTP 确认响应
async fn handle_onebot_event(
    State(state): State<Arc<OneBotInboundState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_authorized(&headers, state.access_token.as_deref()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "status": "failed",
                "retcode": -1,
                "message": "unauthorized"
            })),
        )
            .into_response();
    }
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload) => payload,
        Err(err) => {
            eprintln!(
                "{}: {err:#}",
                t(
                    "【OneBot Gateway】【Invalid payload】",
                    "【OneBot网关】【数据无效】"
                )
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "status": "failed",
                    "retcode": -1,
                    "message": "invalid json"
                })),
            )
                .into_response();
        }
    };
    match parse_message_event(&payload) {
        Ok(Some(event)) => {
            tokio::spawn(async move {
                if let Err(err) = process_message_event(state, event).await {
                    eprintln!(
                        "{}: {err:#}",
                        t(
                            "OneBot inbound event processing failed",
                            "OneBot 入站事件处理失败"
                        )
                    );
                }
            });
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!(
                "{}: {err:#}",
                t("OneBot inbound event ignored", "已忽略 OneBot 入站事件")
            );
        }
    }
    Json(json!({ "status": "ok" })).into_response()
}

/// 校验 OneBot 入站服务配置，避免公网监听时意外启用匿名入口。
///
/// 参数:
/// - `config`: OneBot 入站服务配置
///
/// 返回:
/// - 配置是否满足安全要求
fn validate_server_config(config: &OneBotServerConfig) -> Result<()> {
    let has_token = config
        .access_token
        .as_deref()
        .map(str::trim)
        .is_some_and(|token| !token.is_empty());
    if !has_token && !config.listen.ip().is_loopback() {
        bail!(t(
            "OneBot inbound server requires --access-token when listening beyond loopback",
            "OneBot 入站服务监听非回环地址时必须配置 --access-token"
        ));
    }
    Ok(())
}

/// 校验 OneBot 入站请求中的访问令牌。
///
/// 参数:
/// - `headers`: HTTP 请求头
/// - `expected_token`: 服务端配置的访问令牌
///
/// 返回:
/// - 请求是否通过令牌校验
fn is_authorized(headers: &HeaderMap, expected_token: Option<&str>) -> bool {
    let Some(expected_token) = expected_token
        .map(str::trim)
        .filter(|token| !token.is_empty())
    else {
        return true;
    };
    let authorization = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer_token);
    let legacy_token = headers
        .get("X-Access-Token")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty());
    match (authorization, legacy_token) {
        (Some(authorization), Some(legacy_token)) => {
            constant_time_eq(authorization.as_bytes(), expected_token.as_bytes())
                && constant_time_eq(legacy_token.as_bytes(), expected_token.as_bytes())
        }
        (Some(token), None) | (None, Some(token)) => {
            constant_time_eq(token.as_bytes(), expected_token.as_bytes())
        }
        (None, None) => false,
    }
}

/// 解析 Bearer 访问令牌，认证方案名称不区分大小写。
///
/// 参数:
/// - `value`: Authorization 请求头
///
/// 返回:
/// - Bearer 令牌
fn parse_bearer_token(value: &str) -> Option<&str> {
    let mut parts = value.trim().split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

/// 以常量时间比较两个令牌，减少长度相同令牌的时序差异。
///
/// 参数:
/// - `left`: 第一个字节序列
/// - `right`: 第二个字节序列
///
/// 返回:
/// - 两个字节序列是否相同
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

/// 处理一条 OneBot 消息事件。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `event`: OneBot 消息事件
///
/// 返回:
/// - 处理是否成功
async fn process_message_event(
    state: Arc<OneBotInboundState>,
    event: OneBotMessageEvent,
) -> Result<()> {
    let _guard = state.agent_lock.lock().await;
    let client = OneBotClient::new(
        state.onebot_base_url.clone(),
        state.access_token.clone(),
        event.target_kind,
        event.target_id,
    );
    if let Some(reply) = handle_gateway_command(&state.paths, &event.prompt).await? {
        let message = OutboundMessage {
            text: Some(reply),
            media: Vec::new(),
        };
        client.send(&message).await?;
        return Ok(());
    }
    let (prompt, image_url) = prepare_agent_input(&state, &event).await?;
    let reply = run_agent(&state.paths, prompt, image_url).await?;
    if reply.trim().is_empty() {
        return Ok(());
    }
    let message = OutboundMessage {
        text: Some(reply),
        media: Vec::new(),
    };
    client.send(&message).await?;
    Ok(())
}

/// 准备 Agent 输入文本和首张图片。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `event`: OneBot 消息事件
///
/// 返回:
/// - Agent 文本输入和可选图片 data URL
async fn prepare_agent_input(
    state: &OneBotInboundState,
    event: &OneBotMessageEvent,
) -> Result<(String, Option<String>)> {
    let mut prompt = event.prompt.clone();
    let mut image_url = None;
    for media in &event.media {
        match media.kind {
            OneBotInboundMediaKind::Image if image_url.is_none() => {
                match media_to_data_url(&state.http_client, media).await {
                    Ok(data_url) => image_url = Some(data_url),
                    Err(err) => {
                        prompt.push_str(&format!(
                            "\n\n{}: {err}",
                            t("failed to read image", "图片读取失败")
                        ));
                    }
                }
            }
            OneBotInboundMediaKind::File => {
                if let Ok(path) = download_file_to_cache(state, media).await {
                    prompt.push_str(&format!(
                        "\n\n{}: {}",
                        t("File saved to", "文件已保存到"),
                        path.display()
                    ));
                }
            }
            _ => {}
        }
    }
    Ok((prompt, image_url))
}

/// 运行 Sai Agent 并返回回复文本。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `prompt`: Agent 输入文本
/// - `image_url`: 可选图片 data URL
///
/// 返回:
/// - Agent 回复文本
async fn run_agent(paths: &SaiPaths, prompt: String, image_url: Option<String>) -> Result<String> {
    let mut user_input = crate::runner::UserInputSubmission::new(prompt, AgentMode::Yolo);
    if let Some(image_url) = image_url {
        user_input = user_input.with_image_url(image_url);
    }
    let submission = crate::runner::RunnerSubmission::user_input(
        crate::runner::SubmissionSource::Gateway,
        user_input,
    );
    let output = crate::runner::run_submission(paths, submission).await?;
    let Some(completion) = output.completion else {
        bail!(t(
            "gateway runner completed without assistant content",
            "网关运行完成，但没有助手回复内容"
        ));
    };
    Ok(completion.content)
}

/// 将入站图片转换为 data URL。
///
/// 参数:
/// - `client`: HTTP 客户端
/// - `media`: 入站图片媒体
///
/// 返回:
/// - 图片 data URL
async fn media_to_data_url(client: &reqwest::Client, media: &OneBotInboundMedia) -> Result<String> {
    if media.source.starts_with("data:") {
        return Ok(media.source.clone());
    }
    let (bytes, content_type) = if is_http_url(&media.source) {
        download_bytes(client, &media.source).await?
    } else {
        let path = local_source_path(&media.source);
        let bytes = std::fs::read(&path).with_context(|| {
            format!(
                "{}: {}",
                t("failed to read inbound image", "入站图片读取失败"),
                path.display()
            )
        })?;
        ensure_media_size(bytes.len())?;
        (bytes, content_type_from_path(&path))
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{content_type};base64,{encoded}"))
}

/// 下载入站文件到缓存目录。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `media`: 入站文件媒体
///
/// 返回:
/// - 本地缓存文件路径
async fn download_file_to_cache(
    state: &OneBotInboundState,
    media: &OneBotInboundMedia,
) -> Result<PathBuf> {
    if !is_http_url(&media.source) {
        return Ok(local_source_path(&media.source));
    }
    let (bytes, _) = download_bytes(&state.http_client, &media.source).await?;
    let filename = media
        .name
        .as_deref()
        .map(sanitize_filename)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "onebot-file".to_string());
    let dir = state.paths.cache_dir.join("gateways").join("onebot");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// 下载 HTTP 资源字节。
///
/// 参数:
/// - `client`: HTTP 客户端
/// - `url`: 资源地址
///
/// 返回:
/// - 资源字节和 MIME 类型
async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<(Vec<u8>, String)> {
    let response = client.get(url).send().await.with_context(|| {
        format!(
            "{}: {url}",
            t("failed to download inbound media", "入站媒体下载失败")
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        bail!(
            "{} HTTP {status}",
            t("inbound media returned", "入站媒体请求返回")
        );
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| content_type_from_source(url));
    let bytes = response.bytes().await?.to_vec();
    ensure_media_size(bytes.len())?;
    Ok((bytes, content_type))
}

/// 校验入站媒体大小。
///
/// 参数:
/// - `size`: 字节数
///
/// 返回:
/// - 媒体大小是否合法
fn ensure_media_size(size: usize) -> Result<()> {
    if size > MAX_INBOUND_MEDIA_BYTES {
        bail!(
            "{} {} bytes",
            t("inbound media exceeds", "入站媒体超过限制"),
            MAX_INBOUND_MEDIA_BYTES
        );
    }
    Ok(())
}

/// 判断字符串是否为 HTTP URL。
///
/// 参数:
/// - `source`: 资源地址
///
/// 返回:
/// - 是否为 HTTP URL
fn is_http_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

/// 将 OneBot 本地资源地址转换为文件路径。
///
/// 参数:
/// - `source`: OneBot 资源地址
///
/// 返回:
/// - 本地文件路径
fn local_source_path(source: &str) -> PathBuf {
    source
        .strip_prefix("file://")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source))
}

/// 根据路径推断 MIME 类型。
///
/// 参数:
/// - `path`: 本地路径
///
/// 返回:
/// - MIME 类型
fn content_type_from_path(path: &Path) -> String {
    content_type_from_extension(path.extension().and_then(|value| value.to_str()))
}

/// 根据资源地址推断 MIME 类型。
///
/// 参数:
/// - `source`: 资源地址
///
/// 返回:
/// - MIME 类型
fn content_type_from_source(source: &str) -> String {
    let path = source.split('?').next().unwrap_or(source);
    content_type_from_extension(Path::new(path).extension().and_then(|value| value.to_str()))
}

/// 根据扩展名推断 MIME 类型。
///
/// 参数:
/// - `extension`: 文件扩展名
///
/// 返回:
/// - MIME 类型
fn content_type_from_extension(extension: Option<&str>) -> String {
    match extension.unwrap_or_default().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// 清理缓存文件名。
///
/// 参数:
/// - `name`: 原始文件名
///
/// 返回:
/// - 安全文件名
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn test_state() -> Arc<OneBotInboundState> {
        Arc::new(OneBotInboundState {
            paths: SaiPaths::new().unwrap(),
            onebot_base_url: "http://127.0.0.1:3000".to_string(),
            access_token: Some("secret".to_string()),
            http_client: reqwest::Client::new(),
            agent_lock: Mutex::new(()),
        })
    }

    #[test]
    fn rejects_missing_onebot_token() {
        let headers = headers();
        assert!(!is_authorized(&headers, Some("secret")));
    }

    #[test]
    fn accepts_bearer_and_legacy_onebot_tokens() {
        let mut bearer = headers();
        bearer.insert("Authorization", HeaderValue::from_static("Bearer secret"));
        assert!(is_authorized(&bearer, Some("secret")));

        let mut legacy = headers();
        legacy.insert("X-Access-Token", HeaderValue::from_static("secret"));
        assert!(is_authorized(&legacy, Some("secret")));
    }

    #[test]
    fn rejects_wrong_or_conflicting_onebot_tokens() {
        let mut wrong = headers();
        wrong.insert("Authorization", HeaderValue::from_static("Bearer wrong"));
        assert!(!is_authorized(&wrong, Some("secret")));

        let mut conflicting = headers();
        conflicting.insert("Authorization", HeaderValue::from_static("Bearer secret"));
        conflicting.insert("X-Access-Token", HeaderValue::from_static("wrong"));
        assert!(!is_authorized(&conflicting, Some("secret")));
    }

    #[test]
    fn allows_anonymous_loopback_only() {
        let loopback = OneBotServerConfig {
            listen: "127.0.0.1:8765".parse().unwrap(),
            onebot_base_url: "http://127.0.0.1:3000".to_string(),
            access_token: None,
        };
        validate_server_config(&loopback).unwrap();

        let public = OneBotServerConfig {
            listen: "0.0.0.0:8765".parse().unwrap(),
            onebot_base_url: "http://127.0.0.1:3000".to_string(),
            access_token: None,
        };
        assert!(validate_server_config(&public).is_err());
    }

    #[tokio::test]
    async fn handler_rejects_missing_or_wrong_token() {
        let missing = handle_onebot_event(
            State(test_state()),
            HeaderMap::new(),
            Bytes::from_static(b"{}"),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer wrong"));
        let wrong =
            handle_onebot_event(State(test_state()), headers, Bytes::from_static(b"{}")).await;
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn handler_accepts_correct_token() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer secret"));
        let response =
            handle_onebot_event(State(test_state()), headers, Bytes::from_static(b"{}")).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
