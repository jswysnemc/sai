use super::event::{parse_message_event, parse_validation_event};
use super::processor::{target_kind_name, QqBotProcessor, QqBotProcessorConfig};
use super::signature::{sign_validation, verify_event_signature};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

pub(crate) struct QqBotWebhookServerConfig {
    pub(crate) listen: SocketAddr,
    pub(crate) base_url: String,
    pub(crate) app_id: String,
    pub(crate) client_secret: String,
    pub(crate) verbose: bool,
}

struct QqBotWebhookState {
    client_secret: String,
    processor: Arc<QqBotProcessor>,
}

/// 启动 QQ 官方机器人 Webhook 入站服务。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: QQ Bot Webhook 服务配置
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run_qq_bot_webhook_server(
    paths: &SaiPaths,
    config: QqBotWebhookServerConfig,
) -> Result<()> {
    let listen = config.listen;
    let processor = Arc::new(QqBotProcessor::new(
        paths,
        QqBotProcessorConfig {
            base_url: config.base_url,
            app_id: config.app_id,
            client_secret: config.client_secret.clone(),
            verbose: config.verbose,
        },
    ));
    let state = Arc::new(QqBotWebhookState {
        client_secret: config.client_secret.clone(),
        processor,
    });
    let app = Router::new()
        .route("/", post(handle_qq_bot_webhook))
        .route("/qqbot", post(handle_qq_bot_webhook))
        .with_state(state.clone());
    let listener = TcpListener::bind(listen).await.with_context(|| {
        format!(
            "{}: {listen}",
            t(
                "failed to bind QQ Bot webhook server",
                "无法绑定 QQ Bot Webhook 服务"
            )
        )
    })?;
    println!(
        "{} http://{listen}",
        t(
            "QQ Bot webhook server listening on",
            "QQ Bot Webhook 服务监听地址"
        )
    );
    state
        .processor
        .debug_log(format!("webhook server started listen={listen}"));
    axum::serve(listener, app).await?;
    Ok(())
}

/// 接收 QQ 官方机器人 Webhook 请求。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `headers`: HTTP 请求头
/// - `body`: 原始请求体
///
/// 返回:
/// - QQ Webhook HTTP 响应
async fn handle_qq_bot_webhook(
    State(state): State<Arc<QqBotWebhookState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match handle_qq_bot_webhook_inner(state, headers, body).await {
        Ok(response) => response,
        Err(err) => {
            eprintln!(
                "{}{err:#}",
                t("【QQ Gateway】【Request failed】", "【QQ网关】【请求失败】")
            );
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// 处理 QQ 官方机器人 Webhook 请求。
///
/// 参数:
/// - `state`: 入站服务共享状态
/// - `headers`: HTTP 请求头
/// - `body`: 原始请求体
///
/// 返回:
/// - QQ Webhook HTTP 响应
async fn handle_qq_bot_webhook_inner(
    state: Arc<QqBotWebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let payload = serde_json::from_slice::<Value>(&body)
        .with_context(|| t("invalid QQ payload", "无效的 QQ 数据"))?;
    if let Some(validation) = parse_validation_event(&payload)? {
        state.processor.debug_log(t(
            "received callback URL validation event",
            "收到回调地址验证事件",
        ));
        let signature = sign_validation(
            &state.client_secret,
            &validation.event_ts,
            &validation.plain_token,
        )?;
        return Ok(Json(json!({
            "plain_token": validation.plain_token,
            "signature": signature,
        }))
        .into_response());
    }
    verify_request_signature(&state.client_secret, &headers, &body)?;
    if let Some(event) = parse_message_event(&payload)? {
        state.processor.debug_log(format!(
            "{} event_type={} target_kind={} target_id={} media_count={}",
            t("received message", "收到消息"),
            event.event_type,
            target_kind_name(event.target_kind),
            event.target_id,
            event.media.len()
        ));
        let processor = state.processor.clone();
        tokio::spawn(async move {
            if let Err(err) = processor.handle_message_event(event).await {
                eprintln!(
                    "{}{err:#}",
                    t(
                        "【QQ Gateway】【Message processing failed】",
                        "【QQ网关】【消息处理失败】"
                    )
                );
            }
        });
    }
    Ok(Json(json!({ "op": 12 })).into_response())
}

/// 校验 QQ Webhook 普通事件签名。
///
/// 参数:
/// - `client_secret`: QQ 开放平台 AppSecret
/// - `headers`: HTTP 请求头
/// - `body`: 原始请求体
///
/// 返回:
/// - 签名是否有效
fn verify_request_signature(client_secret: &str, headers: &HeaderMap, body: &[u8]) -> Result<()> {
    let signature = headers
        .get("X-Signature-Ed25519")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            anyhow::anyhow!(t("missing X-Signature-Ed25519", "缺少 X-Signature-Ed25519"))
        })?;
    let timestamp = headers
        .get("X-Signature-Timestamp")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            anyhow::anyhow!(t(
                "missing X-Signature-Timestamp",
                "缺少 X-Signature-Timestamp"
            ))
        })?;
    verify_event_signature(client_secret, timestamp, body, signature)
}
