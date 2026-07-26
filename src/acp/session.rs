use super::client_methods;
use super::client_terminal::TerminalRegistry;
use super::event_bridge::bridge_session_update;
use super::governance::AcpGovernance;
use super::protocol::{self, PROTOCOL_VERSION};
use super::transport::{AcpTransport, PeerMessage, PeerReceiver};
use crate::agent_engine::{EventSender, ExternalTurnEngine, TurnRequest};
use crate::llm::ChatResult;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// 基于 ACP 的外部对话内核。
pub(crate) struct AcpEngine {
    name: String,
    transport: AcpTransport,
    peer: PeerReceiver,
    session_id: Option<String>,
    startup_timeout: Duration,
    governance: AcpGovernance,
    terminals: TerminalRegistry,
    /// 握手响应里的 agentInfo，作为「确实连上了外部内核」的运行时证据
    agent_info: Option<(String, String)>,
    /// agent 是否支持 session/load；不支持时不做恢复尝试
    load_session: bool,
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
            transport,
            peer,
            session_id: None,
            startup_timeout,
            governance,
            terminals: TerminalRegistry::default(),
            agent_info: None,
            load_session: false,
        };
        let (agent_info, load_session) = engine.initialize().await?;
        Ok(Self {
            agent_info: Some(agent_info),
            load_session,
            ..engine
        })
    }

    /// 与 agent 协商协议版本与能力。
    ///
    /// sai 声明自己能提供文件读写与终端，agent 据此决定把这些活儿交回来，
    /// 从而让外部内核的每次落盘与执行都经过 sai 的权限与沙箱。
    ///
    /// 返回:
    /// - `(agent 名称与版本, 是否支持 session/load)`
    async fn initialize(&self) -> Result<((String, String), bool)> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "clientInfo": { "name": "sai", "version": env!("CARGO_PKG_VERSION") },
            "clientCapabilities": {
                "fs": { "readTextFile": true, "writeTextFile": true },
                "terminal": true
            }
        });
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
        let version = response
            .get("protocolVersion")
            .and_then(Value::as_u64)
            .unwrap_or(u64::from(PROTOCOL_VERSION));
        if version != u64::from(PROTOCOL_VERSION) {
            anyhow::bail!(
                "ACP agent speaks protocol version {version}, sai supports {PROTOCOL_VERSION}"
            );
        }
        let info = response.get("agentInfo");
        let name = info
            .and_then(|info| info.get("title").or_else(|| info.get("name")))
            .and_then(Value::as_str)
            .unwrap_or("ACP agent")
            .to_string();
        let version = info
            .and_then(|info| info.get("version"))
            .and_then(Value::as_str)
            .unwrap_or("-")
            .to_string();
        let load_session = response
            .get("agentCapabilities")
            .and_then(|caps| caps.get("loadSession"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(((name, version), load_session))
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
        if self.load_session {
            if let Some(stored) = self
                .governance
                .session_store()
                .and_then(|store| store.load(&self.name))
            {
                let loaded = self
                    .transport
                    .request(
                        "session/load",
                        json!({
                            "sessionId": stored,
                            "cwd": cwd.display().to_string(),
                            "mcpServers": [],
                        }),
                    )
                    .await;
                match loaded {
                    Ok(_) => {
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
        let response = self
            .transport
            .request(
                "session/new",
                json!({ "cwd": cwd.display().to_string(), "mcpServers": [] }),
            )
            .await?;
        let session_id = response
            .get("sessionId")
            .and_then(Value::as_str)
            .context("session/new did not return a sessionId")?
            .to_string();
        if let Some(store) = self.governance.session_store() {
            let _ = store.save(&self.name, &session_id);
        }
        self.session_id = Some(session_id.clone());
        Ok(session_id)
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
    ) -> Result<(String, String)> {
        let prompt = json!({
            "sessionId": session_id,
            "prompt": prompt_blocks(request),
        });
        // session/prompt 的响应要等整轮结束，因此与消息循环并发推进
        let mut prompt_call = Box::pin(self.transport.request("session/prompt", prompt));
        let mut content = String::new();
        let mut reasoning = String::new();
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
                            let bridged = bridge_session_update(&params);
                            content.push_str(&bridged.content_delta);
                            reasoning.push_str(&bridged.reasoning_delta);
                            for event in bridged.events {
                                let _ = events.send(event);
                            }
                        }
                        PeerMessage::Notification { .. } => {}
                        PeerMessage::Request { id, method, params } => {
                            client_methods::handle_peer_request(
                                &self.transport,
                                &id,
                                &method,
                                &params,
                                events,
                                &self.governance,
                                &self.terminals,
                            )
                            .await?;
                        }
                    }
                }
                result = &mut prompt_call => {
                    result?;
                    // prompt 已返回，把已到达的尾部更新排空后收尾
                    while let Ok(Some(message)) = tokio::time::timeout(
                        Duration::from_millis(50),
                        self.peer.recv(),
                    )
                    .await
                    {
                        if let PeerMessage::Notification { method, params } = message {
                            if method != "session/update" {
                                continue;
                            }
                            let bridged = bridge_session_update(&params);
                            content.push_str(&bridged.content_delta);
                            reasoning.push_str(&bridged.reasoning_delta);
                            for event in bridged.events {
                                let _ = events.send(event);
                            }
                        }
                    }
                    return Ok((content, reasoning));
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl ExternalTurnEngine for AcpEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run_turn(
        &mut self,
        request: TurnRequest,
        events: EventSender,
    ) -> Result<ChatResult> {
        let started = Instant::now();
        let session_id = self.ensure_session(&request.cwd).await?;
        let (content, reasoning) = self.drive_prompt(&session_id, &request, &events).await?;
        Ok(ChatResult {
            content,
            reasoning: (!reasoning.is_empty()).then_some(reasoning),
            // ACP 的用量口径与 sai 的估算不同源，不混入本地统计
            usage: None,
            tool_calls: Vec::new(),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    async fn shutdown(&mut self) -> Result<()> {
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
fn prompt_blocks(request: &TurnRequest) -> Value {
    let mut blocks = vec![json!({ "type": "text", "text": request.input })];
    for image in &request.image_urls {
        // data URL 形如 `data:image/png;base64,xxxx`，ACP 要求分开给出 mime 与数据
        let Some((mime, data)) = split_data_url(image) else {
            continue;
        };
        blocks.push(json!({ "type": "image", "mimeType": mime, "data": data }));
    }
    Value::Array(blocks)
}

/// 拆分 data URL 的 MIME 类型与 base64 数据。
///
/// 参数:
/// - `url`: data URL
///
/// 返回:
/// - `(mime, base64)`；不是 base64 data URL 时返回 None
fn split_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    Some((mime.to_string(), data.to_string()))
}

/// 结束会话时通知 agent 取消未完成的轮次。
///
/// 参数:
/// - `transport`: 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 通知结果
#[allow(dead_code)]
pub(crate) async fn cancel(transport: &AcpTransport, session_id: &str) -> Result<()> {
    transport
        .notify("session/cancel", json!({ "sessionId": session_id }))
        .await
}

/// 未实现方法的标准错误码，供客户端方法处理复用。
#[allow(dead_code)]
pub(crate) const METHOD_NOT_FOUND: i64 = protocol::METHOD_NOT_FOUND;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_text_and_images() {
        let request = TurnRequest {
            input: "看这张图".to_string(),
            image_urls: vec!["data:image/png;base64,AAAA".to_string()],
            cwd: std::path::PathBuf::from("/tmp"),
        };
        let blocks = prompt_blocks(&request);
        let array = blocks.as_array().unwrap();
        assert_eq!(array[0]["type"], "text");
        assert_eq!(array[0]["text"], "看这张图");
        assert_eq!(array[1]["type"], "image");
        assert_eq!(array[1]["mimeType"], "image/png");
        assert_eq!(array[1]["data"], "AAAA");
    }

    /// 非 base64 的 data URL 无法拆成 ACP 需要的字段，跳过而不是发出坏数据。
    #[test]
    fn skips_unsupported_image_urls() {
        let request = TurnRequest {
            input: "问题".to_string(),
            image_urls: vec!["https://example.com/a.png".to_string()],
            cwd: std::path::PathBuf::from("/tmp"),
        };
        assert_eq!(prompt_blocks(&request).as_array().unwrap().len(), 1);
    }

    #[test]
    fn splits_base64_data_urls() {
        assert_eq!(
            split_data_url("data:image/jpeg;base64,QUJD"),
            Some(("image/jpeg".to_string(), "QUJD".to_string()))
        );
        assert!(split_data_url("data:text/plain,hello").is_none());
    }
}
