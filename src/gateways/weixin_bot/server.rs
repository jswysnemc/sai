use super::client::WeixinBotClient;
use super::event::{parse_weixin_message, WeixinInboundMediaKind, WeixinMessageEvent};
use super::inbound_media::{image_mime, save_inbound_media, SavedInboundMedia};
use super::prompt::channel_prompt;
use crate::agent::AgentMode;
use crate::cli::build_tool_registry;
use crate::config::AppConfig;
use crate::gateways::channel_context::{save_latest_channel_context, ChannelContext};
use crate::gateways::channel_tools::{register_channel_message_tool, ActiveChannelTarget};
use crate::gateways::command_intercept::handle_gateway_command;
use crate::gateways::session::ensure_gateway_session;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::Mutex;

const RETRY_DELAY: Duration = Duration::from_secs(3);

pub(crate) struct WeixinBotServerConfig {
    pub(crate) base_url: String,
    pub(crate) cdn_base_url: String,
    pub(crate) token: String,
    pub(crate) bot_agent: Option<String>,
    pub(crate) verbose: bool,
}

/// 启动微信官方机器人长轮询服务。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 微信机器人服务配置
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run_weixin_bot_server(
    paths: &SaiPaths,
    config: WeixinBotServerConfig,
) -> Result<()> {
    let client = WeixinBotClient::new(
        config.base_url,
        config.cdn_base_url,
        config.token,
        config.bot_agent,
        config.verbose,
    );
    let http_client = reqwest::Client::new();
    let agent_lock = Mutex::new(());
    let mut updates_buf = None::<String>;
    println!(
        "{}",
        t(
            "Weixin bot long-poll server started",
            "微信机器人长轮询服务已启动"
        )
    );
    client.debug_log(format!(
        "server started base_url={} cdn_base_url={} verbose={}",
        client.base_url(),
        client.cdn_base_url(),
        client.verbose()
    ));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("{}", t(
                    "Weixin bot long-poll server stopped",
                    "微信机器人长轮询服务已停止"
                ));
                return Ok(());
            }
            result = client.get_updates(updates_buf.as_deref()) => {
                match result {
                    Ok(response) => {
                        updates_buf = response
                            .get("get_updates_buf")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                            .or(updates_buf);
                        handle_updates(paths, &client, &http_client, &agent_lock, &response).await?;
                    }
                    Err(err) => {
                        eprintln!("{}: {err:#}", t("Weixin getupdates failed", "微信 getupdates 失败"));
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                }
            }
        }
    }
}

/// 处理 getUpdates 响应。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `client`: 微信 iLink 客户端
/// - `http_client`: HTTP 客户端
/// - `agent_lock`: Agent 串行锁
/// - `response`: getUpdates 响应 JSON
///
/// 返回:
/// - 处理是否成功
async fn handle_updates(
    paths: &SaiPaths,
    client: &WeixinBotClient,
    http_client: &reqwest::Client,
    agent_lock: &Mutex<()>,
    response: &Value,
) -> Result<()> {
    let ret = response.get("ret").and_then(Value::as_i64).unwrap_or(0);
    if ret != 0 {
        bail!(
            "{} ret={ret}: {response}",
            t("Weixin getupdates failed", "微信 getupdates 失败")
        );
    }
    let Some(messages) = response.get("msgs").and_then(Value::as_array) else {
        client.debug_log(t(
            "getupdates response has no message array",
            "getupdates 无消息数组",
        ));
        return Ok(());
    };
    client.debug_log(format!(
        "{}: {}",
        t("getupdates message count", "getupdates 收到消息数"),
        messages.len()
    ));
    for message in messages {
        if let Some(event) = parse_weixin_message(message) {
            client.debug_log(format!(
                "{} from={} media_count={} prompt_chars={}",
                t("processing Weixin message", "处理微信消息"),
                event.from_user_id,
                event.media.len(),
                event.prompt.chars().count()
            ));
            process_message_event(paths, client, http_client, agent_lock, event).await?;
        }
    }
    Ok(())
}

/// 处理一条微信消息事件。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `client`: 微信 iLink 客户端
/// - `http_client`: HTTP 客户端
/// - `agent_lock`: Agent 串行锁
/// - `event`: 微信消息事件
///
/// 返回:
/// - 处理是否成功
async fn process_message_event(
    paths: &SaiPaths,
    client: &WeixinBotClient,
    http_client: &reqwest::Client,
    agent_lock: &Mutex<()>,
    event: WeixinMessageEvent,
) -> Result<()> {
    let _guard = agent_lock.lock().await;
    let context = ChannelContext::weixin(event.from_user_id.clone(), event.context_token.clone());
    save_latest_channel_context(paths, &context)?;
    if let Some(reply) = handle_gateway_command(paths, &event.prompt).await? {
        client
            .send_text(&event.from_user_id, &reply, event.context_token.as_deref())
            .await?;
        return Ok(());
    }
    let (prompt, image_url) = prepare_agent_input(paths, client, http_client, &event).await?;
    let reply = run_agent(paths, client, &event, &context, prompt, image_url).await?;
    if reply.trim().is_empty() {
        return Ok(());
    }
    client
        .send_text(&event.from_user_id, &reply, event.context_token.as_deref())
        .await?;
    Ok(())
}

/// 准备 Agent 输入文本和首张图片。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `weixin_client`: 微信 iLink 客户端
/// - `http_client`: HTTP 客户端
/// - `event`: 微信消息事件
///
/// 返回:
/// - Agent 文本输入和可选图片 data URL
async fn prepare_agent_input(
    paths: &SaiPaths,
    weixin_client: &WeixinBotClient,
    http_client: &reqwest::Client,
    event: &WeixinMessageEvent,
) -> Result<(String, Option<String>)> {
    let mut prompt = event.prompt.clone();
    let mut image_url = None;
    for media in &event.media {
        match save_inbound_media(paths, weixin_client, http_client, media).await {
            Ok(saved) => {
                append_saved_media_prompt(&mut prompt, &saved);
                if saved.kind == WeixinInboundMediaKind::Image && image_url.is_none() {
                    match saved_image_to_data_url(&saved) {
                        Ok(data_url) => image_url = Some(data_url),
                        Err(err) => prompt.push_str(&format!(
                            "\n\n{}: {err}",
                            t(
                                "failed to prepare image vision input",
                                "图片视觉输入准备失败"
                            )
                        )),
                    }
                }
            }
            Err(err) => {
                weixin_client.debug_log(format!(
                    "{} kind={} source={} error={err:#}",
                    t("failed to save inbound attachment", "入站附件保存失败"),
                    media.kind.localized_label(),
                    media.source
                ));
                prompt.push_str(&format!(
                    "\n\n{} {}: {err}\n{}: {}",
                    t("Failed to save user-sent", "用户发送的"),
                    media.kind.localized_label(),
                    t("Source", "来源"),
                    media.source
                ));
            }
        }
    }
    Ok((prompt, image_url))
}

/// 将已保存媒体信息追加到 Agent 输入。
///
/// 参数:
/// - `prompt`: Agent 输入文本
/// - `saved`: 已保存媒体信息
///
/// 返回:
/// - 无
fn append_saved_media_prompt(prompt: &mut String, saved: &SavedInboundMedia) {
    prompt.push_str(&format!(
        "\n\n{} {}: {}\n{}: {}\n{}: {}",
        t("The user sent", "用户发送了"),
        saved.kind.localized_label(),
        saved.name,
        t("Saved to", "已保存到"),
        saved.path.display(),
        t("Media type", "媒体类型"),
        saved.mime_type
    ));
}

/// 运行 Sai Agent 并返回回复文本。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `weixin_client`: 微信 iLink 客户端
/// - `event`: 微信消息事件
/// - `context`: 渠道上下文
/// - `prompt`: Agent 输入文本
/// - `image_url`: 可选图片 data URL
///
/// 返回:
/// - Agent 回复文本
async fn run_agent(
    paths: &SaiPaths,
    weixin_client: &WeixinBotClient,
    event: &WeixinMessageEvent,
    context: &ChannelContext,
    prompt: String,
    image_url: Option<String>,
) -> Result<String> {
    AppConfig::init_files(paths)?;
    let config = crate::config::apply_agent_override(
        AppConfig::load_or_default(paths)?,
        None,
        crate::config::AgentSurface::Gateway,
    )?;
    let mut registry = build_tool_registry(&config, paths, AgentMode::Yolo)?;
    register_channel_message_tool(
        &mut registry,
        paths.clone(),
        config.clone(),
        ActiveChannelTarget::Weixin {
            client: weixin_client.clone(),
            to_user_id: event.from_user_id.clone(),
            context_token: event.context_token.clone(),
        },
    );
    let mut user_input = crate::runner::UserInputSubmission::new(prompt, AgentMode::Yolo)
        .with_extra_system_prompt(channel_prompt());
    if let Some(image_url) = image_url {
        user_input = user_input.with_image_url(image_url);
    }
    let channel = crate::runner::ChannelSubmission::new(context.channel())
        .with_inbound_marker(context.inbound_marker())
        .with_extra_loaded_tool("send_channel_message");
    let session_id = ensure_gateway_session(paths, context)?;
    let submission = crate::runner::RunnerSubmission::user_input(
        crate::runner::SubmissionSource::Gateway,
        user_input,
    )
    .with_session_id(session_id)
    .with_channel(channel);
    let output = run_gateway_submission(paths, config, registry, submission).await?;
    let Some(completion) = output.completion else {
        bail!(t(
            "gateway runner completed without assistant content",
            "网关运行完成，但没有助手回复内容"
        ));
    };
    Ok(completion.content)
}

/// 通过 runner 执行 gateway submission。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `registry`: 工具注册表
/// - `submission`: runner submission
///
/// 返回:
/// - runner 输出
async fn run_gateway_submission(
    paths: &SaiPaths,
    config: AppConfig,
    registry: crate::tools::ToolRegistry,
    submission: crate::runner::RunnerSubmission,
) -> Result<crate::runner::RunnerOutput> {
    let mut output = crate::runner::RunnerOutput::default();
    let mut sink = |event| {
        output.push_event(event);
        Ok(())
    };
    crate::runner::SessionRunner::new(paths)
        .with_config(config)
        .with_tool_registry(registry)
        .run_submission(submission, &mut sink)
        .await?;
    Ok(output)
}

/// 将已保存图片转换为 data URL。
///
/// 参数:
/// - `saved`: 已保存媒体信息
///
/// 返回:
/// - 图片 data URL
fn saved_image_to_data_url(saved: &SavedInboundMedia) -> Result<String> {
    let bytes = std::fs::read(&saved.path).with_context(|| {
        format!(
            "{}: {}",
            t(
                "failed to read saved Weixin image",
                "读取已保存微信图片失败"
            ),
            saved.path.display()
        )
    })?;
    let content_type = image_mime(&bytes).ok_or_else(|| {
        anyhow::anyhow!(t(
            "the saved Weixin image format is not supported by the model",
            "已保存微信图片不是模型支持的图片格式"
        ))
    })?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{content_type};base64,{encoded}"))
}
