use super::auth::QqBotAuthenticator;
use super::event::{QqBotInboundMediaKind, QqBotMessageEvent};
use super::inbound_media::{save_inbound_media, saved_image_to_data_url, SavedQqInboundMedia};
use super::prompt::channel_prompt;
use crate::agent::AgentMode;
use crate::cli::build_tool_registry;
use crate::config::AppConfig;
use crate::gateways::channel_context::{save_latest_channel_context, ChannelContext};
use crate::gateways::channel_tools::{register_channel_message_tool, ActiveChannelTarget};
use crate::gateways::command_intercept::handle_gateway_command;
use crate::gateways::message::OutboundMessage;
use crate::gateways::qq_official::{QqOfficialClient, QqTargetKind};
use crate::gateways::session::ensure_gateway_session;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use anyhow::{bail, Result};
use serde_json::Value;
use tokio::sync::Mutex;

pub(crate) struct QqBotProcessorConfig {
    pub(crate) base_url: String,
    pub(crate) app_id: String,
    pub(crate) client_secret: String,
    pub(crate) verbose: bool,
}

pub(crate) struct QqBotProcessor {
    paths: SaiPaths,
    base_url: String,
    verbose: bool,
    authenticator: Mutex<QqBotAuthenticator>,
    http_client: reqwest::Client,
    agent_lock: Mutex<()>,
}

impl QqBotProcessor {
    /// 创建 QQ 官方机器人消息处理器。
    ///
    /// 参数:
    /// - `paths`: Sai 路径
    /// - `config`: QQ 官方机器人处理配置
    ///
    /// 返回:
    /// - QQ 官方机器人消息处理器
    pub(crate) fn new(paths: &SaiPaths, config: QqBotProcessorConfig) -> Self {
        Self {
            paths: paths.clone(),
            base_url: config.base_url,
            verbose: config.verbose,
            authenticator: Mutex::new(QqBotAuthenticator::new(config.app_id, config.client_secret)),
            http_client: reqwest::Client::new(),
            agent_lock: Mutex::new(()),
        }
    }

    /// 输出 QQ 网关调试日志。
    ///
    /// 参数:
    /// - `message`: 日志内容
    ///
    /// 返回:
    /// - 无
    pub(crate) fn debug_log(&self, message: impl AsRef<str>) {
        if self.verbose {
            eprintln!(
                "{}{}",
                t("【QQ Gateway】【Debug】", "【QQ网关】【调试】"),
                message.as_ref()
            );
        }
    }

    /// 记录 QQ WebSocket transport 断开观察事件。
    ///
    /// 参数:
    /// - `reason`: 断开原因
    /// - `last_sequence`: 最近一次 Gateway Dispatch 序号
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_websocket_transport_close(
        &self,
        reason: &str,
        last_sequence: Option<u64>,
    ) -> Result<()> {
        let state = StateStore::new(&self.paths)?;
        state.record_gateway_transport_close("qq", reason, last_sequence)
    }

    /// 推进 QQ WebSocket transport cursor 和 ack。
    ///
    /// 参数:
    /// - `cursor_seq`: 可选已接收 Gateway Dispatch 序号
    /// - `acked_seq`: 可选已处理 Gateway Dispatch 序号
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn advance_websocket_transport_cursor(
        &self,
        cursor_seq: Option<u64>,
        acked_seq: Option<u64>,
    ) -> Result<()> {
        let state = StateStore::new(&self.paths)?;
        state.advance_gateway_transport_cursor("qq", cursor_seq, acked_seq)?;
        Ok(())
    }

    /// 审计 QQ WebSocket 是否存在无法 replay 的未确认区间。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否写入恢复记录
    pub(crate) fn audit_websocket_transport_replay(&self) -> Result<bool> {
        let state = StateStore::new(&self.paths)?;
        state.audit_gateway_transport_replay("qq")
    }

    /// 写入 QQ WebSocket transport 事件到本地 replay source。
    ///
    /// 参数:
    /// - `sequence`: Gateway Dispatch 序号
    /// - `payload`: 原始 Gateway Payload
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_websocket_transport_event(
        &self,
        sequence: u64,
        payload: &Value,
    ) -> Result<()> {
        let state = StateStore::new(&self.paths)?;
        state.record_gateway_transport_event("qq", sequence, payload)
    }

    /// 读取 QQ WebSocket 本地 replay 事件。
    ///
    /// 参数:
    /// - `start_sequence`: 起始序号
    /// - `end_sequence`: 结束序号
    ///
    /// 返回:
    /// - 按序排列的 Gateway Payload
    pub(crate) fn load_websocket_transport_replay_events(
        &self,
        start_sequence: i64,
        end_sequence: i64,
    ) -> Result<Vec<Value>> {
        let state = StateStore::new(&self.paths)?;
        let events = state.load_gateway_transport_events("qq", start_sequence, end_sequence)?;
        events
            .into_iter()
            .map(|event| serde_json::from_str::<Value>(&event.payload_json).map_err(Into::into))
            .collect()
    }

    /// 开始应用 QQ WebSocket replay 事件。
    ///
    /// 参数:
    /// - `sequence`: Gateway Dispatch 序号
    ///
    /// 返回:
    /// - replay 应用决策
    pub(crate) fn begin_websocket_transport_replay_event(
        &self,
        sequence: u64,
    ) -> Result<crate::runtime_recovery::RuntimeTransportReplayDecision> {
        let state = StateStore::new(&self.paths)?;
        state.begin_gateway_transport_replay_event("qq", sequence)
    }

    /// 处理一条 QQ 官方机器人消息事件。
    ///
    /// 参数:
    /// - `event`: QQ 消息事件
    ///
    /// 返回:
    /// - 处理是否成功
    pub(crate) async fn handle_message_event(&self, event: QqBotMessageEvent) -> Result<()> {
        let _guard = self.agent_lock.lock().await;
        let context = ChannelContext::qq(
            event.target_kind,
            event.target_id.clone(),
            Some(event.msg_id.clone()),
        );
        save_latest_channel_context(&self.paths, &context)?;
        let authorization = self.authenticator.lock().await.authorization().await?;
        let client = QqOfficialClient::new(
            self.base_url.clone(),
            authorization,
            event.target_kind,
            event.target_id.clone(),
        );
        if let Some(reply) = handle_gateway_command(&self.paths, &event.prompt).await? {
            let message = OutboundMessage {
                text: Some(reply),
                media: Vec::new(),
            };
            client.send(&message, Some(&event.msg_id)).await?;
            return Ok(());
        }
        let (prompt, image_url) = self.prepare_agent_input(&event).await?;
        let reply = self
            .run_agent(client.clone(), &event, &context, prompt, image_url)
            .await?;
        if reply.trim().is_empty() {
            return Ok(());
        }
        let message = OutboundMessage {
            text: Some(reply),
            media: Vec::new(),
        };
        client.send(&message, Some(&event.msg_id)).await?;
        Ok(())
    }

    /// 准备 Agent 输入文本和首张图片。
    ///
    /// 参数:
    /// - `event`: QQ 消息事件
    ///
    /// 返回:
    /// - Agent 文本输入和可选图片 data URL
    async fn prepare_agent_input(
        &self,
        event: &QqBotMessageEvent,
    ) -> Result<(String, Option<String>)> {
        let mut prompt = event.prompt.clone();
        let mut image_url = None;
        for media in &event.media {
            match save_inbound_media(&self.paths, &self.http_client, media).await {
                Ok(saved) => {
                    append_saved_media_prompt(&mut prompt, &saved);
                    self.debug_log(format!(
                        "{} kind={} name={} mime={} path={}",
                        t("inbound attachment saved", "入站附件已保存"),
                        inbound_media_name(saved.kind),
                        saved.name,
                        saved.mime_type,
                        saved.path.display()
                    ));
                    if saved.kind == QqBotInboundMediaKind::Image && image_url.is_none() {
                        match saved_image_to_data_url(&saved) {
                            Ok(data_url) => image_url = Some(data_url),
                            Err(err) => {
                                prompt.push_str(&format!(
                                    "\n\n{}: {err}",
                                    t(
                                        "failed to prepare image vision input",
                                        "图片视觉输入准备失败"
                                    )
                                ));
                            }
                        }
                    }
                }
                Err(err) => {
                    self.debug_log(format!(
                        "{} kind={} source={} error={err:#}",
                        t("failed to save inbound attachment", "入站附件保存失败"),
                        inbound_media_name(media.kind),
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

    /// 运行 Sai Agent 并返回回复文本。
    ///
    /// 参数:
    /// - `qq_client`: QQ 官方机器人客户端
    /// - `event`: QQ 消息事件
    /// - `prompt`: Agent 输入文本
    /// - `image_url`: 可选图片 data URL
    ///
    /// 返回:
    /// - Agent 回复文本
    async fn run_agent(
        &self,
        qq_client: QqOfficialClient,
        event: &QqBotMessageEvent,
        context: &ChannelContext,
        prompt: String,
        image_url: Option<String>,
    ) -> Result<String> {
        AppConfig::init_files(&self.paths)?;
        let config = crate::config::apply_agent_override(
            AppConfig::load_or_default(&self.paths)?,
            None,
            crate::config::AgentSurface::Gateway,
        )?;
        let mut registry = build_tool_registry(&config, &self.paths, AgentMode::Yolo)?;
        register_channel_message_tool(
            &mut registry,
            self.paths.clone(),
            config.clone(),
            ActiveChannelTarget::Qq {
                client: qq_client,
                msg_id: Some(event.msg_id.clone()),
            },
        );
        let user_input = gateway_user_input(prompt, image_url, Some(channel_prompt()));
        let channel = crate::runner::ChannelSubmission::new(context.channel())
            .with_inbound_marker(context.inbound_marker())
            .with_extra_loaded_tool("send_channel_message");
        let session_id = ensure_gateway_session(&self.paths, context)?;
        let submission = crate::runner::RunnerSubmission::user_input(
            crate::runner::SubmissionSource::Gateway,
            user_input,
        )
        .with_session_id(session_id)
        .with_channel(channel);
        let output = run_gateway_submission(&self.paths, config, registry, submission).await?;
        let Some(completion) = output.completion else {
            bail!(t(
                "gateway runner completed without assistant content",
                "网关运行完成，但没有助手回复内容"
            ));
        };
        Ok(completion.content)
    }
}

/// 构造 gateway 用户输入 submission。
///
/// 参数:
/// - `prompt`: Agent 输入文本
/// - `image_url`: 可选图片 data URL
/// - `extra_system_prompt`: 可选额外系统提示词
///
/// 返回:
/// - 用户输入 submission
fn gateway_user_input(
    prompt: String,
    image_url: Option<String>,
    extra_system_prompt: Option<&str>,
) -> crate::runner::UserInputSubmission {
    let mut input = crate::runner::UserInputSubmission::new(prompt, AgentMode::Yolo);
    if let Some(image_url) = image_url {
        input = input.with_image_url(image_url);
    }
    if let Some(prompt) = extra_system_prompt {
        input = input.with_extra_system_prompt(prompt);
    }
    input
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

/// 将已保存媒体信息追加到 Agent 输入。
///
/// 参数:
/// - `prompt`: Agent 输入文本
/// - `saved`: 已保存媒体信息
///
/// 返回:
/// - 无
fn append_saved_media_prompt(prompt: &mut String, saved: &SavedQqInboundMedia) {
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

/// 返回 QQ 目标类型名称。
///
/// 参数:
/// - `kind`: QQ 目标类型
///
/// 返回:
/// - 类型名称
pub(crate) fn target_kind_name(kind: QqTargetKind) -> &'static str {
    match kind {
        QqTargetKind::User => "user",
        QqTargetKind::Group => "group",
    }
}

/// 返回入站媒体日志名称。
///
/// 参数:
/// - `kind`: 入站媒体类型
///
/// 返回:
/// - 媒体类型文本
fn inbound_media_name(kind: QqBotInboundMediaKind) -> &'static str {
    match kind {
        QqBotInboundMediaKind::Image => "image",
        QqBotInboundMediaKind::Voice => "voice",
        QqBotInboundMediaKind::Video => "video",
        QqBotInboundMediaKind::File => "file",
    }
}
