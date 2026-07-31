use super::client::FeishuClient;
use super::event::{parse_inbound, FeishuInbound, FeishuMessage};
use crate::config::AppConfig;
use crate::gateways::channel_context::{save_latest_channel_context, ChannelContext};
use crate::gateways::command_intercept::handle_gateway_command;
use crate::gateways::session::ensure_gateway_session;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 飞书网关运行配置。
pub(crate) struct FeishuBotServerConfig {
    pub(crate) listen: String,
    pub(crate) base_url: String,
    pub(crate) app_id: String,
    pub(crate) app_secret: String,
    pub(crate) verification_token: String,
    pub(crate) encrypt_key: String,
    pub(crate) verbose: bool,
}

/// 服务端共享状态。
struct FeishuState {
    paths: SaiPaths,
    client: FeishuClient,
    verification_token: String,
    encrypt_key: String,
    verbose: bool,
    /// 同一时刻只跑一轮：会话状态与工作区都是单份，并发会互相踩
    agent_lock: Mutex<()>,
}

/// 启动飞书事件订阅服务。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 网关配置
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run_feishu_bot_server(
    paths: &SaiPaths,
    config: FeishuBotServerConfig,
) -> Result<()> {
    let listen = config.listen.clone();
    let state = Arc::new(FeishuState {
        paths: paths.clone(),
        client: FeishuClient::new(&config.base_url, &config.app_id, &config.app_secret),
        verification_token: config.verification_token,
        encrypt_key: config.encrypt_key,
        verbose: config.verbose,
        agent_lock: Mutex::new(()),
    });
    let app = Router::new()
        .route("/", post(handle_event))
        .route("/feishu", post(handle_event))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("failed to bind the Feishu gateway to {listen}"))?;
    println!("feishu gateway listening on {listen}");
    axum::serve(listener, app)
        .await
        .context("the Feishu gateway stopped unexpectedly")?;
    Ok(())
}

/// 处理一条事件订阅推送。
///
/// 飞书要求快速回 200，否则会重试推送同一事件；
/// 因此这里只做校验与解析，实际对话放到后台任务里跑。
///
/// 参数:
/// - `state`: 共享状态
/// - `body`: 原始请求体
///
/// 返回:
/// - HTTP 响应
async fn handle_event(State(state): State<Arc<FeishuState>>, body: String) -> impl IntoResponse {
    let inbound = match parse_inbound(&body, &state.encrypt_key) {
        Ok(inbound) => inbound,
        Err(error) => {
            if state.verbose {
                eprintln!("feishu event parse failed: {error:#}");
            }
            return (StatusCode::BAD_REQUEST, Json(json!({ "code": 1 })));
        }
    };
    match inbound {
        // 地址验证必须原样回 challenge，开放平台据此确认回调可用
        FeishuInbound::UrlVerification { challenge } => {
            (StatusCode::OK, Json(json!({ "challenge": challenge })))
        }
        FeishuInbound::Ignored => (StatusCode::OK, Json(json!({ "code": 0 }))),
        FeishuInbound::Message(message) => {
            if !token_matches(&state, &body) {
                if state.verbose {
                    eprintln!("feishu event rejected: verification token mismatch");
                }
                return (StatusCode::UNAUTHORIZED, Json(json!({ "code": 1 })));
            }
            let task_state = Arc::clone(&state);
            tokio::spawn(async move {
                if let Err(error) = process_message(task_state.clone(), message).await {
                    eprintln!("feishu message handling failed: {error:#}");
                }
            });
            (StatusCode::OK, Json(json!({ "code": 0 })))
        }
    }
}

/// 校验事件来源。
///
/// 开启加密时 token 在密文内，解密成功本身已证明来源；
/// 未配置 token 则不校验，便于本地联调。
///
/// 参数:
/// - `state`: 共享状态
/// - `body`: 原始请求体
///
/// 返回:
/// - 校验通过时为 true
fn token_matches(state: &FeishuState, body: &str) -> bool {
    if state.verification_token.trim().is_empty() || !state.encrypt_key.trim().is_empty() {
        return true;
    }
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/header/token")
                .or_else(|| value.get("token"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|token| token == state.verification_token)
}

/// 跑一轮对话并把结果回到会话里。
///
/// 参数:
/// - `state`: 共享状态
/// - `message`: 收到的消息
///
/// 返回:
/// - 处理结果
async fn process_message(state: Arc<FeishuState>, message: FeishuMessage) -> Result<()> {
    let _guard = state.agent_lock.lock().await;
    let context = ChannelContext::feishu(message.chat_id.clone(), Some(message.message_id.clone()));
    save_latest_channel_context(&state.paths, &context)?;
    // 1. 网关命令（如 /new）直接回复，不进入对话
    if let Some(reply) = handle_gateway_command(&state.paths, &message.text).await? {
        state.client.send_text(&message.chat_id, &reply).await?;
        return Ok(());
    }
    let reply = run_turn(&state, &context, &message).await?;
    if reply.trim().is_empty() {
        return Ok(());
    }
    state.client.send_text(&message.chat_id, &reply).await?;
    Ok(())
}

/// 以网关身份执行一轮对话。
///
/// 参数:
/// - `state`: 共享状态
/// - `context`: 渠道上下文
/// - `message`: 收到的消息
///
/// 返回:
/// - 助手回复
async fn run_turn(
    state: &FeishuState,
    context: &ChannelContext,
    message: &FeishuMessage,
) -> Result<String> {
    AppConfig::init_files(&state.paths)?;
    let config = crate::config::apply_agent_override(
        AppConfig::load_or_default(&state.paths)?,
        None,
        crate::config::AgentSurface::Gateway,
    )?;
    let registry =
        crate::cli::build_tool_registry(&config, &state.paths, crate::agent::AgentMode::Yolo)?;
    let user_input = crate::runner::UserInputSubmission::new(
        message.text.clone(),
        crate::agent::AgentMode::Yolo,
    );
    let channel = crate::runner::ChannelSubmission::new(context.channel())
        .with_inbound_marker(context.inbound_marker());
    let session_id = ensure_gateway_session(&state.paths, context)?;
    let submission = crate::runner::RunnerSubmission::user_input(
        crate::runner::SubmissionSource::Gateway,
        user_input,
    )
    .with_session_id(session_id)
    .with_channel(channel);
    let mut output = crate::runner::RunnerOutput::default();
    let mut sink = |event| {
        output.push_event(event);
        Ok(())
    };
    crate::runner::SessionRunner::new(&state.paths)
        .with_config(config)
        .with_tool_registry(registry)
        .run_submission(submission, &mut sink)
        .await?;
    let Some(completion) = output.completion else {
        anyhow::bail!("the Feishu gateway run finished without assistant content")
    };
    Ok(completion.content)
}
