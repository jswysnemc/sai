use super::client_methods;
use super::client_terminal::TerminalRegistry;
use super::config_options::AcpConfigOptions;
use super::event_bridge::AcpEventBridge;
use super::governance::AcpGovernance;
use super::protocol;
use super::transport::{AcpTransport, PeerMessage, PeerReceiver};
use crate::agent_engine::{EventSender, ExternalTurnEngine, TurnRequest};
use crate::llm::ChatResult;
use agent_client_protocol::schema::v1::{
    AuthMethod, AuthenticateRequest, AuthenticateResponse, CloseSessionRequest,
    CloseSessionResponse, ContentBlock, DeleteSessionRequest, DeleteSessionResponse, ImageContent,
    ListSessionsRequest, ListSessionsResponse, LoadSessionRequest, LoadSessionResponse,
    LogoutRequest, LogoutResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, ResumeSessionRequest, ResumeSessionResponse, SessionConfigOption, SessionInfo,
    SessionModeState, SetSessionModeRequest, SetSessionModeResponse, TextContent,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::time::{Duration, Instant};

/// Codex app-server 保留的非来源客户端名称。
///
/// 使用该名称时，app-server 不会用 ACP 宿主覆盖默认的 `codex_cli_rs`
/// 来源和 User-Agent，行为与 Sai 内置内核的 Codex 客户端兼容模式一致。
const CODEX_APP_SERVER_NON_ORIGINATING_CLIENT: &str = "codex_app_server_daemon";

/// 返回外部内核在 ACP 握手中声明的客户端名称。
///
/// 参数:
/// - `engine_name`: 外部内核稳定名称
///
/// 返回:
/// - 发送给 ACP agent 的 `clientInfo.name`
pub(super) fn client_info_name(engine_name: &str) -> &'static str {
    if engine_name == "codex" {
        CODEX_APP_SERVER_NON_ORIGINATING_CLIENT
    } else {
        "sai"
    }
}

/// 基于 ACP 的外部对话内核。
pub(crate) struct AcpEngine {
    name: String,
    transport: std::sync::Arc<AcpTransport>,
    peer: PeerReceiver,
    session_id: Option<String>,
    startup_timeout: Duration,
    governance: AcpGovernance,
    terminals: TerminalRegistry,
    /// 维护 ACP 工具调用标识与展示名称的关联
    event_bridge: AcpEventBridge,
    /// 握手响应里的 agentInfo，作为「确实连上了外部内核」的运行时证据
    agent_info: Option<(String, String)>,
    /// agent 是否支持 session/load；不支持时不做恢复尝试
    capabilities: super::capabilities::AcpCapabilities,
    /// 当前会话公开的标准配置项
    config_options: AcpConfigOptions,
    /// 当前连接是否执行过显式 authenticate
    authenticated: bool,
}

impl AcpEngine {
    /// 启动外部 agent 并完成握手。
    ///
    /// 参数:
    /// - `name`: 内核标识
    /// - `program`: 启动程序
    /// - `args`: 启动参数
    /// - `env`: 追加环境变量
    /// - `cwd`: 工作目录
    /// - `startup_timeout`: 握手超时
    /// - `governance`: 治理句柄
    ///
    /// 返回:
    /// - 已完成 initialize 的内核
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn connect(
        name: &str,
        program: &str,
        args: &[String],
        env: &std::collections::BTreeMap<String, String>,
        cwd: &std::path::Path,
        startup_timeout: Duration,
        governance: AcpGovernance,
    ) -> Result<Self> {
        let (transport, peer) = AcpTransport::spawn(program, args, env, cwd).await?;
        let engine = Self {
            name: name.to_string(),
            transport: std::sync::Arc::new(transport),
            peer,
            session_id: None,
            startup_timeout,
            governance,
            terminals: TerminalRegistry::default(),
            event_bridge: AcpEventBridge::default(),
            agent_info: None,
            capabilities: Default::default(),
            config_options: Default::default(),
            authenticated: false,
        };
        let initialized = engine.initialize().await?;
        let authenticated = engine.authenticate(&initialized.auth_methods).await?;
        super::capabilities::publish(name, &initialized.capabilities, &initialized.auth_methods);
        Ok(Self {
            agent_info: Some((initialized.name, initialized.version)),
            capabilities: initialized.capabilities,
            authenticated,
            ..engine
        })
    }

    /// 与 agent 协商协议版本与能力。
    ///
    /// sai 声明自己能提供文件读写与终端，agent 据此决定把这些活儿交回来，
    /// 从而让外部内核的每次落盘与执行都经过 sai 的权限与沙箱。
    ///
    /// 返回:
    /// - agent 名称、版本和协商后的能力
    async fn initialize(&self) -> Result<super::capabilities::InitializedAgent> {
        let client_name = client_info_name(&self.name);
        let params = super::sdk::initialize_params(client_name)?;
        // 首次运行要下载适配器，握手超时给得比普通请求宽
        let response = tokio::time::timeout(
            self.startup_timeout,
            self.transport.request("initialize", params),
        )
        .await
        .with_context(|| {
            format!(
                "ACP agent did not finish initialize within {}s",
                self.startup_timeout.as_secs()
            )
        })??;
        super::capabilities::parse_initialize_response(response)
    }

    /// 按配置选择 agent 公布的认证方式。
    ///
    /// 参数:
    /// - `methods`: initialize 响应中的认证方式
    ///
    /// 返回:
    /// - 是否执行了显式认证
    async fn authenticate(&self, methods: &[AuthMethod]) -> Result<bool> {
        let method_id = self.governance.config().agent.acp.auth_method.trim();
        if method_id.is_empty() {
            return Ok(false);
        }
        if !methods
            .iter()
            .any(|method| method.id().to_string() == method_id)
        {
            anyhow::bail!("ACP agent does not advertise auth method: {method_id}");
        }
        let request = AuthenticateRequest::new(method_id.to_string());
        let response = self
            .transport
            .request("authenticate", super::sdk::to_value(&request)?)
            .await?;
        let _: AuthenticateResponse = super::sdk::from_value(response, "authenticate response")?;
        Ok(true)
    }

    /// 取出握手得到的 agent 名称与版本。
    ///
    /// 返回:
    /// - agentInfo；尚未握手时为 None
    pub(crate) fn agent_info(&self) -> Option<(String, String)> {
        self.agent_info.clone()
    }

    /// 确保存在可用会话。
    ///
    /// 参数:
    /// - `cwd`: 会话根目录
    ///
    /// 返回:
    /// - 会话标识
    async fn ensure_session(&mut self, cwd: &std::path::Path) -> Result<String> {
        if let Some(session_id) = &self.session_id {
            return Ok(session_id.clone());
        }
        // 1. 先尝试接回上次的会话：外部 agent 自管历史，
        //    新建会话意味着丢掉全部上下文，与 sai 会话列表里的历史对不上
        if self.capabilities.resume_session || self.capabilities.load_session {
            if let Some(stored) = self
                .governance
                .session_store()
                .and_then(|store| store.load(&self.name))
            {
                let loaded = self.restore_session(&stored, cwd).await;
                match loaded {
                    Ok(setup) => {
                        // 【ACP】【会话恢复】session/load 会把历史内容作为 session/update 重放；
                        // 响应已经说明恢复完成，这批通知不能进入下一轮增量
                        self.peer.discard_session_updates();
                        self.config_options.replace(setup.config_options);
                        self.apply_config_options(&stored).await?;
                        self.apply_legacy_mode(&stored, setup.modes.as_ref())
                            .await?;
                        self.publish_session_state(setup.modes.as_ref());
                        self.session_id = Some(stored.clone());
                        return Ok(stored);
                    }
                    Err(_) => {
                        // 标识已失效（会话被对端清理等），丢弃后新建，
                        // 否则每次启动都拿同一个坏标识去试
                        if let Some(store) = self.governance.session_store() {
                            store.clear();
                        }
                    }
                }
            }
        }
        // 2. 没有可恢复的会话时新建，并记下标识供下次接回
        let context = super::session_context::build(&self.governance, &self.capabilities)?;
        let request = NewSessionRequest::new(cwd)
            .mcp_servers(context.mcp_servers)
            .additional_directories(context.additional_directories)
            .meta(context.meta);
        let response = self
            .transport
            .request("session/new", super::sdk::to_value(&request)?)
            .await?;
        let response: NewSessionResponse =
            super::sdk::from_value(response, "session/new response")?;
        let session_id = response.session_id.to_string();
        let modes = response.modes;
        self.config_options.replace(response.config_options);
        self.apply_config_options(&session_id).await?;
        self.apply_legacy_mode(&session_id, modes.as_ref()).await?;
        self.publish_session_state(modes.as_ref());
        if let Some(store) = self.governance.session_store() {
            let _ = store.save(&self.name, &session_id);
        }
        self.session_id = Some(session_id.clone());
        Ok(session_id)
    }

    /// 使用 agent 支持的最佳方法恢复持久化会话。
    ///
    /// 参数:
    /// - `session_id`: 持久化的 ACP 会话标识
    /// - `cwd`: 会话工作目录
    ///
    /// 返回:
    /// - 恢复后的配置项与旧版模式
    async fn restore_session(
        &self,
        session_id: &str,
        cwd: &std::path::Path,
    ) -> Result<RestoredSession> {
        let context = super::session_context::build(&self.governance, &self.capabilities)?;
        if self.capabilities.resume_session {
            let request = ResumeSessionRequest::new(session_id.to_string(), cwd)
                .mcp_servers(context.mcp_servers)
                .additional_directories(context.additional_directories)
                .meta(context.meta);
            let response = self
                .transport
                .request("session/resume", super::sdk::to_value(&request)?)
                .await?;
            let response: ResumeSessionResponse =
                super::sdk::from_value(response, "session/resume response")?;
            return Ok(RestoredSession {
                config_options: response.config_options,
                modes: response.modes,
            });
        }
        let request = LoadSessionRequest::new(session_id.to_string(), cwd)
            .mcp_servers(context.mcp_servers)
            .additional_directories(context.additional_directories)
            .meta(context.meta);
        let response = self
            .transport
            .request("session/load", super::sdk::to_value(&request)?)
            .await?;
        let response: LoadSessionResponse =
            super::sdk::from_value(response, "session/load response")?;
        Ok(RestoredSession {
            config_options: response.config_options,
            modes: response.modes,
        })
    }

    /// 将配置文件中的 ACP 配置覆盖应用到当前会话。
    ///
    /// 参数:
    /// - `session_id`: 当前 ACP 会话标识
    ///
    /// 返回:
    /// - 配置更新结果
    async fn apply_config_options(&mut self, session_id: &str) -> Result<()> {
        let config = self.governance.config().agent.acp.clone();
        self.config_options
            .apply_configured_values(&self.transport, session_id, &config)
            .await
    }

    /// 在 agent 仍使用旧版 modes 时应用权限模式配置。
    ///
    /// 参数:
    /// - `session_id`: 当前 ACP 会话标识
    /// - `modes`: agent 返回的旧版模式集合
    ///
    /// 返回:
    /// - 模式设置结果
    async fn apply_legacy_mode(
        &self,
        session_id: &str,
        modes: Option<&SessionModeState>,
    ) -> Result<()> {
        let configured = self.governance.config().agent.acp.permission_mode.trim();
        if configured.is_empty()
            || self.config_options.options().iter().any(|option| {
                matches!(
                    option.category,
                    Some(agent_client_protocol::schema::v1::SessionConfigOptionCategory::Mode)
                )
            })
        {
            return Ok(());
        }
        let Some(modes) = modes else {
            return Ok(());
        };
        if !modes
            .available_modes
            .iter()
            .any(|mode| mode.id.to_string() == configured)
        {
            anyhow::bail!("ACP agent does not expose session mode: {configured}");
        }
        if modes.current_mode_id.to_string() == configured {
            return Ok(());
        }
        let request = SetSessionModeRequest::new(session_id.to_string(), configured.to_string());
        let response = self
            .transport
            .request("session/set_mode", super::sdk::to_value(&request)?)
            .await?;
        let _: SetSessionModeResponse =
            super::sdk::from_value(response, "session/set_mode response")?;
        Ok(())
    }

    /// 发布当前会话的配置项与旧版模式，供前端动态构造控制项。
    ///
    /// 参数:
    /// - `modes`: agent 返回的旧版模式集合
    ///
    /// 返回:
    /// - 无
    fn publish_session_state(&self, modes: Option<&SessionModeState>) {
        super::runtime_state::publish_session(&self.name, self.config_options.options(), modes);
    }

    /// 驱动一轮提示，直到 agent 给出停止原因。
    ///
    /// 期间要同时处理两类消息：agent 推送的 `session/update` 通知，
    /// 以及它反向发起的文件、终端与权限请求。
    ///
    /// 参数:
    /// - `session_id`: 会话标识
    /// - `request`: 本轮输入
    /// - `events`: 事件发送端
    ///
    /// 返回:
    /// - 本轮累计的回复与推理
    async fn drive_prompt(
        &mut self,
        session_id: &str,
        request: &TurnRequest,
        events: &EventSender,
    ) -> Result<(String, String, Option<crate::llm::Usage>)> {
        let prompt = PromptRequest::new(
            session_id.to_string(),
            prompt_blocks(request, &self.capabilities)?,
        );
        // 本轮 future 被丢弃时（用户中断、上游 abort）通知对端取消，
        // 否则外部 agent 仍在跑这一轮，下次提示会撞上残留状态
        let mut cancel_guard = CancelOnDrop {
            transport: std::sync::Arc::clone(&self.transport),
            session_id: session_id.to_string(),
            host_session_id: self.governance.session_id().to_string(),
            finished: false,
        };
        // session/prompt 的响应要等整轮结束，因此与消息循环并发推进
        let mut prompt_call = Box::pin(
            self.transport
                .request("session/prompt", super::sdk::to_value(&prompt)?),
        );
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut reported_usage = None;
        loop {
            tokio::select! {
                // 优先处理对端消息，避免 prompt 先返回时丢掉尾部更新
                biased;
                message = self.peer.recv() => {
                    let Some(message) = message else {
                        anyhow::bail!("ACP agent closed the connection during the turn");
                    };
                    match message {
                        PeerMessage::Notification { method, params } if method == "session/update" => {
                            let bridged = self.event_bridge.bridge_session_update(&params);
                            self.config_options.replace(bridged.config_options.clone());
                            if let Some(options) = &bridged.config_options {
                                super::runtime_state::update_config_options(&self.name, options);
                            }
                            if let Some(mode) = &bridged.current_mode {
                                super::runtime_state::update_current_mode(&self.name, mode);
                            }
                            if bridged.usage.is_some() {
                                reported_usage = bridged.usage.clone();
                            }
                            content.push_str(&bridged.content_delta);
                            reasoning.push_str(&bridged.reasoning_delta);
                            for event in bridged.events {
                                let _ = events.send(event);
                            }
                        }
                        PeerMessage::Notification { .. } => {}
                        PeerMessage::Request { id, method, params } => {
                            self.dispatch_peer_request(id, method, params, events);
                        }
                    }
                }
                result = &mut prompt_call => {
                    let response: PromptResponse = super::sdk::from_value(
                        result?,
                        "session/prompt response",
                    )?;
                    // prompt 已返回，把已到达的尾部更新排空后收尾
                    while let Ok(Some(message)) = tokio::time::timeout(
                        Duration::from_millis(50),
                        self.peer.recv(),
                    )
                    .await
                    {
                        match message {
                            PeerMessage::Notification { method, params } => {
                                if method != "session/update" {
                                    continue;
                                }
                                let bridged = self.event_bridge.bridge_session_update(&params);
                                self.config_options.replace(bridged.config_options.clone());
                                if let Some(options) = &bridged.config_options {
                                    super::runtime_state::update_config_options(&self.name, options);
                                }
                                if let Some(mode) = &bridged.current_mode {
                                    super::runtime_state::update_current_mode(&self.name, mode);
                                }
                                if bridged.usage.is_some() {
                                    reported_usage = bridged.usage.clone();
                                }
                                content.push_str(&bridged.content_delta);
                                reasoning.push_str(&bridged.reasoning_delta);
                                for event in bridged.events {
                                    let _ = events.send(event);
                                }
                            }
                            PeerMessage::Request { id, method, params } => {
                                self.dispatch_peer_request(id, method, params, events);
                            }
                        }
                    }
                    cancel_guard.finished = true;
                    let usage = response.usage.map(convert_usage).or(reported_usage);
                    return Ok((content, reasoning, usage));
                }
            }
        }
    }

    /// 并行处理 agent 反向发起的客户端请求。
    ///
    /// 权限与征询可能等待用户数分钟，不能阻塞 session/update、取消和其它终端请求。
    ///
    /// 参数:
    /// - `id`: 对端请求标识
    /// - `method`: ACP 方法名
    /// - `params`: 请求参数
    /// - `events`: 当前轮次事件发送端
    ///
    /// 返回:
    /// - 无
    fn dispatch_peer_request(
        &self,
        id: super::protocol::RequestId,
        method: String,
        params: serde_json::Value,
        events: &EventSender,
    ) {
        let transport = std::sync::Arc::clone(&self.transport);
        let governance = self.governance.clone();
        let terminals = self.terminals.clone();
        let events = events.clone();
        tokio::spawn(async move {
            let _ = client_methods::handle_peer_request(
                &transport,
                &id,
                &method,
                &params,
                &events,
                &governance,
                &terminals,
            )
            .await;
        });
    }

    /// 列出 agent 保存的会话。
    ///
    /// 参数:
    /// - `cwd`: 可选工作目录过滤条件
    /// - `cursor`: 可选分页游标
    ///
    /// 返回:
    /// - 会话列表与下一页游标
    #[allow(dead_code)]
    pub(crate) async fn list_sessions(
        &self,
        cwd: Option<std::path::PathBuf>,
        cursor: Option<String>,
    ) -> Result<(Vec<SessionInfo>, Option<String>)> {
        if !self.capabilities.list_sessions {
            anyhow::bail!("ACP agent does not advertise session/list");
        }
        let request = ListSessionsRequest::new().cwd(cwd).cursor(cursor);
        let response = self
            .transport
            .request("session/list", super::sdk::to_value(&request)?)
            .await?;
        let response: ListSessionsResponse =
            super::sdk::from_value(response, "session/list response")?;
        Ok((response.sessions, response.next_cursor))
    }

    /// 删除 agent 保存的会话。
    ///
    /// 参数:
    /// - `session_id`: 待删除的 ACP 会话标识
    ///
    /// 返回:
    /// - 删除结果
    #[allow(dead_code)]
    pub(crate) async fn delete_session(&self, session_id: &str) -> Result<()> {
        if !self.capabilities.delete_session {
            anyhow::bail!("ACP agent does not advertise session/delete");
        }
        let request = DeleteSessionRequest::new(session_id.to_string());
        let response = self
            .transport
            .request("session/delete", super::sdk::to_value(&request)?)
            .await?;
        let _: DeleteSessionResponse = super::sdk::from_value(response, "session/delete response")?;
        Ok(())
    }

    /// 退出 agent 维护的认证状态。
    ///
    /// 返回:
    /// - agent 支持 logout 且当前连接显式认证过时发送请求
    #[allow(dead_code)]
    pub(crate) async fn logout(&mut self) -> Result<()> {
        if !self.authenticated {
            return Ok(());
        }
        if !self.capabilities.logout {
            anyhow::bail!("ACP agent does not advertise logout");
        }
        let response = self
            .transport
            .request("logout", super::sdk::to_value(&LogoutRequest::new())?)
            .await?;
        let _: LogoutResponse = super::sdk::from_value(response, "logout response")?;
        self.authenticated = false;
        Ok(())
    }
}

/// 会话恢复响应中的可配置状态。
struct RestoredSession {
    config_options: Option<Vec<SessionConfigOption>>,
    modes: Option<SessionModeState>,
}

/// 轮次未正常结束时向对端发出取消。
///
/// sai 的中断是直接 abort 承载轮次的 tokio 任务，future 就此被丢弃；
/// 若不补发 `session/cancel`，外部 agent 会继续跑完这一轮，
/// 其产生的工具调用与文件改动都不再受这边约束。
struct CancelOnDrop {
    transport: std::sync::Arc<AcpTransport>,
    session_id: String,
    host_session_id: String,
    finished: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        crate::permission::discard_pending_permissions_for_session(&self.host_session_id);
        crate::question::discard_pending_questions_for_session(&self.host_session_id);
        // Drop 不能 await，交给运行时另起任务发送；
        // 运行时正在关闭时拿不到句柄，此时子进程也会随之回收，无需再取消
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let transport = std::sync::Arc::clone(&self.transport);
        let session_id = self.session_id.clone();
        handle.spawn(async move {
            let _ = transport
                .notify("session/cancel", json!({ "sessionId": session_id }))
                .await;
        });
    }
}

#[async_trait::async_trait]
impl ExternalTurnEngine for AcpEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run_turn(&mut self, request: TurnRequest, events: EventSender) -> Result<ChatResult> {
        let started = Instant::now();
        let session_id = self.ensure_session(&request.cwd).await?;
        let (content, reasoning, usage) = self.drive_prompt(&session_id, &request, &events).await?;
        Ok(ChatResult {
            content,
            reasoning: (!reasoning.is_empty()).then_some(reasoning),
            usage,
            tool_calls: Vec::new(),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn shutdown(&mut self) -> Result<()> {
        if self.capabilities.close_session {
            if let Some(session_id) = self.session_id.take() {
                let request = CloseSessionRequest::new(session_id);
                let response = self
                    .transport
                    .request("session/close", super::sdk::to_value(&request)?)
                    .await?;
                let _: CloseSessionResponse =
                    super::sdk::from_value(response, "session/close response")?;
            }
        }
        self.transport.shutdown().await
    }
}

/// 把本轮输入转成 ACP 的 content block 数组。
///
/// 参数:
/// - `request`: 本轮输入
///
/// 返回:
/// - content block 数组
pub(super) fn prompt_blocks(
    request: &TurnRequest,
    capabilities: &super::capabilities::AcpCapabilities,
) -> Result<Vec<ContentBlock>> {
    let mut blocks = vec![ContentBlock::Text(TextContent::new(&request.input))];
    if !request.image_urls.is_empty() && !capabilities.prompt_image {
        anyhow::bail!("ACP agent does not advertise image prompt support");
    }
    for image in &request.image_urls {
        // data URL 形如 `data:image/png;base64,xxxx`，ACP 要求分开给出 mime 与数据
        let Some((mime, data)) = split_data_url(image) else {
            continue;
        };
        blocks.push(ContentBlock::Image(ImageContent::new(data, mime)));
    }
    Ok(blocks)
}

/// 将 ACP turn usage 转换为 Sai 的统一用量结构。
///
/// 参数:
/// - `usage`: agent 在 prompt 响应中报告的用量
///
/// 返回:
/// - Sai 统一用量
fn convert_usage(usage: agent_client_protocol::schema::v1::Usage) -> crate::llm::Usage {
    crate::llm::Usage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
        cache_read_tokens: usage.cached_read_tokens.unwrap_or_default(),
        cache_write_tokens: usage.cached_write_tokens.unwrap_or_default(),
    }
}

/// 拆分 data URL 的 MIME 类型与 base64 数据。
///
/// 参数:
/// - `url`: data URL
///
/// 返回:
/// - `(mime, base64)`；不是 base64 data URL 时返回 None
pub(super) fn split_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    Some((mime.to_string(), data.to_string()))
}

/// 未实现方法的标准错误码，供客户端方法处理复用。
#[allow(dead_code)]
pub(crate) const METHOD_NOT_FOUND: i64 = protocol::METHOD_NOT_FOUND;
