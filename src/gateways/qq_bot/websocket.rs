use super::auth::QqBotAuthenticator;
use super::event::parse_message_event;
use super::processor::{target_kind_name, QqBotProcessor, QqBotProcessorConfig};
use crate::paths::SaiPaths;
use crate::runtime_recovery::RuntimeTransportReplayDecision;
use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::USER_AGENT;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

const RECONNECT_DELAY: Duration = Duration::from_secs(3);
const QQ_GATEWAY_OP_DISPATCH: i64 = 0;
const QQ_GATEWAY_OP_HEARTBEAT: i64 = 1;
const QQ_GATEWAY_OP_IDENTIFY: i64 = 2;
const QQ_GATEWAY_OP_HELLO: i64 = 10;
const QQ_DIRECT_MESSAGE_INTENT: u64 = 1u64 << 12;
const QQ_GROUP_AND_C2C_INTENT: u64 = 1u64 << 25;
const QQ_INTERACTION_INTENT: u64 = 1u64 << 26;
const QQ_PUBLIC_GUILD_MESSAGES_INTENT: u64 = 1u64 << 30;
const QQ_FULL_INTENTS: u64 = QQ_PUBLIC_GUILD_MESSAGES_INTENT
    | QQ_DIRECT_MESSAGE_INTENT
    | QQ_GROUP_AND_C2C_INTENT
    | QQ_INTERACTION_INTENT;
const QQ_GATEWAY_USER_AGENT: &str = "Sai QQBot Gateway";

type QqWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

enum WebsocketReplayAction {
    ApplyCurrent,
    ApplyBuffered(Vec<Value>),
    Skip,
}

pub(crate) struct QqBotWebsocketConfig {
    pub(crate) base_url: String,
    pub(crate) app_id: String,
    pub(crate) client_secret: String,
    pub(crate) verbose: bool,
}

#[derive(Debug, Deserialize)]
struct GatewayUrlResponse {
    url: String,
}

/// 启动 QQ 官方机器人 WebSocket 出站网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: QQ Bot WebSocket 配置
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run_qq_bot_websocket(
    paths: &SaiPaths,
    config: QqBotWebsocketConfig,
) -> Result<()> {
    let base_url = config.base_url.trim_end_matches('/').to_string();
    let processor = Arc::new(QqBotProcessor::new(
        paths,
        QqBotProcessorConfig {
            base_url: base_url.clone(),
            app_id: config.app_id.clone(),
            client_secret: config.client_secret.clone(),
            verbose: config.verbose,
        },
    ));
    let http_client = reqwest::Client::new();
    let mut authenticator = QqBotAuthenticator::new(config.app_id, config.client_secret);
    println!("QQ Bot websocket gateway started");
    processor.debug_log(format!(
        "websocket gateway started base_url={} verbose={}",
        base_url, config.verbose
    ));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("QQ Bot websocket gateway stopped");
                return Ok(());
            }
            result = run_websocket_once(&base_url, &http_client, &mut authenticator, processor.clone()) => {
                match result {
                    Ok(()) => eprintln!("【QQ网关】【WebSocket断开】连接已关闭，准备重连"),
                    Err(err) => eprintln!("【QQ网关】【WebSocket失败】{err:#}"),
                }
                audit_websocket_transport_replay(&processor);
                tokio::time::sleep(RECONNECT_DELAY).await;
            }
        }
    }
}

/// 建立并运行一次 QQ WebSocket 会话。
///
/// 参数:
/// - `base_url`: QQ OpenAPI 基础地址
/// - `http_client`: HTTP 客户端
/// - `authenticator`: QQ 官方机器人认证器
/// - `processor`: QQ 消息处理器
///
/// 返回:
/// - 单次连接运行结果
async fn run_websocket_once(
    base_url: &str,
    http_client: &reqwest::Client,
    authenticator: &mut QqBotAuthenticator,
    processor: Arc<QqBotProcessor>,
) -> Result<()> {
    let access_token = authenticator.access_token().await?;
    let gateway_url = get_gateway_url(base_url, http_client, &access_token).await?;
    processor.debug_log(format!("连接 WebSocket gateway_url={gateway_url}"));
    let mut request = gateway_url
        .as_str()
        .into_client_request()
        .with_context(|| format!("invalid QQ websocket gateway URL: {gateway_url}"))?;
    request
        .headers_mut()
        .insert(USER_AGENT, HeaderValue::from_static(QQ_GATEWAY_USER_AGENT));
    let (mut websocket, _) = connect_async(request)
        .await
        .with_context(|| format!("failed to connect QQ websocket gateway: {gateway_url}"))?;
    let heartbeat_interval = read_hello(&mut websocket).await?;
    send_identify(&mut websocket, &access_token).await?;
    run_dispatch_loop(websocket, heartbeat_interval, processor).await
}

/// 获取 QQ WebSocket Gateway 地址。
///
/// 参数:
/// - `base_url`: QQ OpenAPI 基础地址
/// - `client`: HTTP 客户端
/// - `access_token`: QQ access token
///
/// 返回:
/// - WebSocket Gateway 地址
async fn get_gateway_url(
    base_url: &str,
    client: &reqwest::Client,
    access_token: &str,
) -> Result<String> {
    let url = format!("{base_url}/gateway");
    let response = client
        .get(&url)
        .header("Authorization", format!("QQBot {access_token}"))
        .send()
        .await
        .with_context(|| format!("failed to request QQ gateway URL: {url}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("QQ gateway URL API returned HTTP {status}: {body}");
    }
    let parsed = serde_json::from_str::<GatewayUrlResponse>(&body)
        .with_context(|| format!("invalid QQ gateway URL response: {body}"))?;
    let gateway_url = parsed.url.trim();
    if gateway_url.is_empty() {
        bail!("QQ gateway URL response has empty url");
    }
    Ok(gateway_url.to_string())
}

/// 读取 QQ Gateway Hello 帧。
///
/// 参数:
/// - `websocket`: WebSocket 连接
///
/// 返回:
/// - 心跳间隔
async fn read_hello(websocket: &mut QqWebSocket) -> Result<Duration> {
    loop {
        let payload = read_json_message(websocket).await?;
        let Some(payload) = payload else {
            continue;
        };
        if payload.get("op").and_then(Value::as_i64) == Some(QQ_GATEWAY_OP_HELLO) {
            return heartbeat_interval(&payload);
        }
    }
}

/// 发送 QQ Gateway Identify 帧。
///
/// 参数:
/// - `websocket`: WebSocket 连接
/// - `access_token`: QQ access token
///
/// 返回:
/// - 发送是否成功
async fn send_identify(websocket: &mut QqWebSocket, access_token: &str) -> Result<()> {
    let payload = json!({
        "op": QQ_GATEWAY_OP_IDENTIFY,
        "d": {
            "token": format!("QQBot {access_token}"),
            "intents": QQ_FULL_INTENTS,
            "shard": [0, 1],
        }
    });
    websocket
        .send(Message::Text(payload.to_string().into()))
        .await
        .context("failed to send QQ websocket identify")?;
    Ok(())
}

/// 运行 QQ Gateway Dispatch 和 Heartbeat 循环。
///
/// 参数:
/// - `websocket`: WebSocket 连接
/// - `heartbeat_interval`: 心跳间隔
/// - `processor`: QQ 消息处理器
///
/// 返回:
/// - 会话运行结果
async fn run_dispatch_loop(
    mut websocket: QqWebSocket,
    heartbeat_interval: Duration,
    processor: Arc<QqBotProcessor>,
) -> Result<()> {
    let mut heartbeat = tokio::time::interval(heartbeat_interval);
    heartbeat.tick().await;
    let mut last_sequence = None::<u64>;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if let Err(err) = send_heartbeat(&mut websocket, last_sequence).await {
                    record_websocket_transport_close(
                        &processor,
                        &format!("QQ websocket heartbeat failed: {err:#}"),
                        last_sequence,
                    );
                    return Err(err);
                }
            }
            payload = read_json_message(&mut websocket) => {
                let payload = match payload {
                    Ok(Some(payload)) => payload,
                    Ok(None) => continue,
                    Err(err) => {
                        record_websocket_transport_close(
                            &processor,
                            &format!("QQ websocket receive failed: {err:#}"),
                            last_sequence,
                        );
                        return Err(err);
                    }
                };
                let current_sequence = payload.get("s").and_then(Value::as_u64);
                if let Some(sequence) = current_sequence {
                    last_sequence = Some(sequence);
                    record_websocket_transport_event(&processor, sequence, &payload);
                    match begin_websocket_transport_replay_event(&processor, sequence) {
                        WebsocketReplayAction::ApplyCurrent => {
                            handle_gateway_payload(payload, processor.clone()).await?;
                            advance_websocket_transport_cursor(&processor, None, Some(sequence));
                        }
                        WebsocketReplayAction::ApplyBuffered(payloads) => {
                            for payload in payloads {
                                let sequence = payload.get("s").and_then(Value::as_u64);
                                handle_gateway_payload(payload, processor.clone()).await?;
                                if let Some(sequence) = sequence {
                                    advance_websocket_transport_cursor(
                                        &processor,
                                        None,
                                        Some(sequence),
                                    );
                                }
                            }
                        }
                        WebsocketReplayAction::Skip => {}
                    }
                    continue;
                }
                handle_gateway_payload(payload, processor.clone()).await?;
            }
        }
    }
}

/// 发送 QQ Gateway Heartbeat 帧。
///
/// 参数:
/// - `websocket`: WebSocket 连接
/// - `last_sequence`: 最近一次 Dispatch 序号
///
/// 返回:
/// - 发送是否成功
async fn send_heartbeat(websocket: &mut QqWebSocket, last_sequence: Option<u64>) -> Result<()> {
    let payload = json!({
        "op": QQ_GATEWAY_OP_HEARTBEAT,
        "d": last_sequence,
    });
    websocket
        .send(Message::Text(payload.to_string().into()))
        .await
        .context("failed to send QQ websocket heartbeat")?;
    Ok(())
}

/// 记录 QQ WebSocket transport 断开，失败时只输出错误。
///
/// 参数:
/// - `processor`: QQ 消息处理器
/// - `reason`: 断开原因
/// - `last_sequence`: 最近一次 Gateway Dispatch 序号
///
/// 返回:
/// - 无
fn record_websocket_transport_close(
    processor: &QqBotProcessor,
    reason: &str,
    last_sequence: Option<u64>,
) {
    // 1. WebSocket reconnect 是 transport 边界，不能复用进程关闭策略终止网关进程
    if let Err(err) = processor.record_websocket_transport_close(reason, last_sequence) {
        eprintln!("【QQ网关】【恢复记录失败】{err:#}");
    }
}

/// 推进 QQ WebSocket transport cursor 或 ack，失败时只输出错误。
///
/// 参数:
/// - `processor`: QQ 消息处理器
/// - `cursor_seq`: 可选已接收 Gateway Dispatch 序号
/// - `acked_seq`: 可选已处理 Gateway Dispatch 序号
///
/// 返回:
/// - 无
fn advance_websocket_transport_cursor(
    processor: &QqBotProcessor,
    cursor_seq: Option<u64>,
    acked_seq: Option<u64>,
) {
    // 1. cursor/ack 是 transport 恢复边界，不写入 conversation turn
    if let Err(err) = processor.advance_websocket_transport_cursor(cursor_seq, acked_seq) {
        eprintln!("【QQ网关】【游标记录失败】{err:#}");
    }
}

/// 审计 QQ WebSocket transport 是否存在无法 replay 的未确认区间。
///
/// 参数:
/// - `processor`: QQ 消息处理器
///
/// 返回:
/// - 无
fn audit_websocket_transport_replay(processor: &QqBotProcessor) {
    // 1. QQ Gateway 当前没有 replay 请求实现，只能把未确认区间暴露为恢复记录
    if let Err(err) = processor.audit_websocket_transport_replay() {
        eprintln!("【QQ网关】【重放审计失败】{err:#}");
    }
}

/// 写入 QQ WebSocket transport 事件到本地 replay source。
///
/// 参数:
/// - `processor`: QQ 消息处理器
/// - `sequence`: Gateway Dispatch 序号
/// - `payload`: 原始 Gateway Payload
///
/// 返回:
/// - 无
fn record_websocket_transport_event(processor: &QqBotProcessor, sequence: u64, payload: &Value) {
    // 1. transport payload 先落本地 inbox，后续 gap 才能从本地 replay source 恢复
    if let Err(err) = processor.record_websocket_transport_event(sequence, payload) {
        eprintln!("【QQ网关】【重放事件记录失败】{err:#}");
    }
}

/// 开始应用 QQ WebSocket transport replay 事件。
///
/// 参数:
/// - `processor`: QQ 消息处理器
/// - `sequence`: Gateway Dispatch 序号
///
/// 返回:
/// - replay 处理动作
fn begin_websocket_transport_replay_event(
    processor: &QqBotProcessor,
    sequence: u64,
) -> WebsocketReplayAction {
    match processor.begin_websocket_transport_replay_event(sequence) {
        Ok(RuntimeTransportReplayDecision::Apply { .. }) => WebsocketReplayAction::ApplyCurrent,
        Ok(RuntimeTransportReplayDecision::ReplayBuffered {
            replay_start,
            replay_end,
            ..
        }) => match processor.load_websocket_transport_replay_events(replay_start, replay_end) {
            Ok(payloads) => WebsocketReplayAction::ApplyBuffered(payloads),
            Err(err) => {
                eprintln!("【QQ网关】【重放读取失败】{err:#}");
                WebsocketReplayAction::Skip
            }
        },
        Ok(RuntimeTransportReplayDecision::SkipStale {
            sequence,
            acked_seq,
        }) => {
            eprintln!(
                "【QQ网关】【重放跳过】跳过已确认事件 sequence={sequence} acked_seq={acked_seq}"
            );
            WebsocketReplayAction::Skip
        }
        Ok(RuntimeTransportReplayDecision::GapUnavailable {
            sequence,
            missing_start,
            missing_end,
            acked_seq,
        }) => {
            eprintln!(
                "【QQ网关】【重放缺口】跳过缺口后的事件 sequence={sequence} missing={missing_start}..{missing_end} acked_seq={acked_seq}"
            );
            WebsocketReplayAction::Skip
        }
        Err(err) => {
            eprintln!("【QQ网关】【重放状态失败】{err:#}");
            WebsocketReplayAction::ApplyCurrent
        }
    }
}

/// 读取并解析 WebSocket JSON 消息。
///
/// 参数:
/// - `websocket`: WebSocket 连接
///
/// 返回:
/// - 可选 JSON 消息
async fn read_json_message(websocket: &mut QqWebSocket) -> Result<Option<Value>> {
    let message = websocket
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("QQ websocket closed"))?
        .context("failed to read QQ websocket message")?;
    match message {
        Message::Text(text) => {
            let value = serde_json::from_str::<Value>(&text)
                .with_context(|| format!("invalid QQ websocket text payload: {text}"))?;
            Ok(Some(value))
        }
        Message::Binary(bytes) => {
            let value = serde_json::from_slice::<Value>(&bytes)
                .context("invalid QQ websocket binary payload")?;
            Ok(Some(value))
        }
        Message::Ping(bytes) => {
            websocket
                .send(Message::Pong(bytes))
                .await
                .context("failed to send QQ websocket pong")?;
            Ok(None)
        }
        Message::Pong(_) => Ok(None),
        Message::Close(frame) => {
            bail!("QQ websocket closed by server: {frame:?}");
        }
        _ => Ok(None),
    }
}

/// 处理 QQ Gateway Payload。
///
/// 参数:
/// - `payload`: Gateway Payload
/// - `processor`: QQ 消息处理器
///
/// 返回:
/// - 处理是否成功
async fn handle_gateway_payload(payload: Value, processor: Arc<QqBotProcessor>) -> Result<()> {
    if payload.get("op").and_then(Value::as_i64) != Some(QQ_GATEWAY_OP_DISPATCH) {
        return Ok(());
    }
    if let Some(event) = parse_message_event(&payload)? {
        processor.debug_log(format!(
            "收到 WebSocket 消息 event_type={} target_kind={} target_id={} media_count={}",
            event.event_type,
            target_kind_name(event.target_kind),
            event.target_id,
            event.media.len()
        ));
        tokio::spawn(async move {
            if let Err(err) = processor.handle_message_event(event).await {
                eprintln!("【QQ网关】【消息处理失败】{err:#}");
            }
        });
    }
    Ok(())
}

/// 读取 QQ Gateway 心跳间隔。
///
/// 参数:
/// - `payload`: Hello Payload
///
/// 返回:
/// - 心跳间隔
fn heartbeat_interval(payload: &Value) -> Result<Duration> {
    let interval_ms = payload
        .get("d")
        .and_then(|data| data.get("heartbeat_interval"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("QQ websocket hello has no heartbeat_interval"))?;
    Ok(Duration::from_millis(interval_ms))
}
