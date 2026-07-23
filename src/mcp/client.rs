use crate::config::McpServerConfig;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

mod environment;
#[path = "naming.rs"]
mod naming;
mod protocol;
mod runtime;

use environment::{expand_env_map, expand_env_value};
pub use naming::dynamic_tool_name;
use protocol::{matches_id, parse_rpc_body, parse_sse_endpoint};
pub(super) use runtime::list_server_tools_on_rt;
pub use runtime::{
    block_on_mcp, call_server_tool, list_server_tools, runtime_statuses, stop_all_servers,
    stop_server, test_server,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub struct McpRuntimeStatus {
    pub server_id: String,
    #[allow(dead_code)]
    pub transport: String,
    pub running: bool,
    pub initialized: bool,
    pub last_error: Option<String>,
}

enum Transport {
    Stdio {
        #[allow(dead_code)]
        child: Child,
        stdin: ChildStdin,
        reader: BufReader<ChildStdout>,
    },
    Http {
        client: reqwest::Client,
        url: String,
        headers: HashMap<String, String>,
        session_id: Option<String>,
    },
    Sse {
        client: reqwest::Client,
        message_url: String,
        headers: HashMap<String, String>,
        session_id: Option<String>,
    },
}

struct PooledClient {
    config_fingerprint: String,
    transport: Transport,
    initialized: bool,
    last_error: Option<String>,
}

impl PooledClient {
    /// 创建并初始化一个 MCP 池化客户端。
    ///
    /// 参数:
    /// - `config`: MCP 服务器配置
    ///
    /// 返回:
    /// - 已完成协议初始化的客户端
    async fn connect(config: &McpServerConfig) -> Result<Self> {
        let transport_name = config.transport.trim().to_ascii_lowercase();
        let transport = match transport_name.as_str() {
            "http" => Transport::connect_http(config).await?,
            "sse" => Transport::connect_sse(config).await?,
            _ => Transport::connect_stdio(config).await?,
        };
        let mut client = Self {
            config_fingerprint: runtime::fingerprint(config),
            transport,
            initialized: false,
            last_error: None,
        };
        client.initialize(config.timeout_ms).await?;
        Ok(client)
    }

    /// 协商 MCP 协议版本并发送初始化完成通知。
    ///
    /// 参数:
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - 初始化成功时返回空值
    async fn initialize(&mut self, timeout_ms: Option<u64>) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        let candidates = [
            "2025-11-25",
            "2025-06-18",
            "2025-03-26",
            "2024-11-05",
            "2024-10-07",
        ];
        let mut last_err = None;
        for version in candidates {
            match self
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": version,
                        "capabilities": {},
                        "clientInfo": {
                            "name": "sai",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                    timeout_ms,
                )
                .await
            {
                Ok(_) => {
                    let _ = self
                        .notify("notifications/initialized", json!({}), timeout_ms)
                        .await;
                    self.initialized = true;
                    self.last_error = None;
                    return Ok(());
                }
                Err(error) => last_err = Some(error),
            }
        }
        let error = last_err.unwrap_or_else(|| anyhow::anyhow!("initialize failed"));
        self.last_error = Some(error.to_string());
        Err(error)
    }

    /// 发送带请求 ID 的 MCP JSON-RPC 请求。
    ///
    /// 参数:
    /// - `method`: JSON-RPC 方法名
    /// - `params`: 请求参数
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - JSON-RPC result 字段
    async fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout_ms: Option<u64>,
    ) -> Result<Value> {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.transport.request(id, payload, timeout_ms).await
    }

    /// 发送不需要响应的 MCP JSON-RPC 通知。
    ///
    /// 参数:
    /// - `method`: JSON-RPC 方法名
    /// - `params`: 通知参数
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - 通知发送结果
    async fn notify(&mut self, method: &str, params: Value, timeout_ms: Option<u64>) -> Result<()> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.transport.notify(payload, timeout_ms).await
    }

    /// 读取 MCP 服务器提供的工具定义。
    ///
    /// 参数:
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - 工具名称、说明和输入 Schema 列表
    async fn list_tools(
        &mut self,
        timeout_ms: Option<u64>,
    ) -> Result<Vec<(String, String, Value)>> {
        let result = self.request("tools/list", json!({}), timeout_ms).await?;
        let tools = result
            .get("tools")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let description = tool
                .get("description")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            let schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| json!({"type":"object","properties":{}}));
            out.push((name, description, schema));
        }
        Ok(out)
    }

    /// 调用一个 MCP 工具并提取文本结果。
    ///
    /// 参数:
    /// - `name`: 远端工具名称
    /// - `arguments`: 工具参数
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - 工具文本结果
    async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
        timeout_ms: Option<u64>,
    ) -> Result<String> {
        let result = self
            .request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
                timeout_ms,
            )
            .await?;
        if result
            .get("isError")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return Ok(format!("mcp tool error: {result}"));
        }
        if let Some(content) = result.get("content").and_then(|value| value.as_array()) {
            let mut texts = Vec::new();
            for item in content {
                if item.get("type").and_then(|value| value.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                        texts.push(text.to_string());
                    }
                }
            }
            if !texts.is_empty() {
                return Ok(texts.join("\n"));
            }
        }
        Ok(result.to_string())
    }
}

impl Transport {
    /// 启动 stdio MCP 子进程并接管标准输入输出。
    ///
    /// 参数:
    /// - `config`: MCP 服务器配置
    ///
    /// 返回:
    /// - stdio 传输实例
    async fn connect_stdio(config: &McpServerConfig) -> Result<Self> {
        if config.command.trim().is_empty() {
            bail!("mcp server {} missing command", config.id);
        }
        let mut command = Command::new(&config.command);
        let args: Vec<String> = config
            .args
            .iter()
            .map(|arg| expand_env_value(arg))
            .collect();
        let env = expand_env_map(&config.env);
        command
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = config
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            command.current_dir(cwd);
        }
        for (key, value) in &env {
            command.env(key, value);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn mcp server {}", config.id))?;
        let stdin = child.stdin.take().context("mcp stdin")?;
        let stdout = child.stdout.take().context("mcp stdout")?;
        Ok(Self::Stdio {
            child,
            stdin,
            reader: BufReader::new(stdout),
        })
    }

    /// 创建 Streamable HTTP MCP 传输。
    ///
    /// 参数:
    /// - `config`: MCP 服务器配置
    ///
    /// 返回:
    /// - HTTP 传输实例
    async fn connect_http(config: &McpServerConfig) -> Result<Self> {
        let url = config
            .url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("mcp server {} missing url", config.id))?
            .to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(
                config.timeout_ms.unwrap_or(60_000).clamp(500, 300_000),
            ))
            .build()?;
        Ok(Self::Http {
            client,
            url,
            headers: expand_env_map(&config.headers),
            session_id: None,
        })
    }

    /// 创建经典 SSE MCP 传输并解析消息端点。
    ///
    /// 参数:
    /// - `config`: MCP 服务器配置
    ///
    /// 返回:
    /// - SSE 传输实例
    async fn connect_sse(config: &McpServerConfig) -> Result<Self> {
        let url = config
            .url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("mcp server {} missing url", config.id))?
            .to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(
                config.timeout_ms.unwrap_or(60_000).clamp(500, 300_000),
            ))
            .build()?;
        // 1. 若配置了 message_url，直接使用
        if let Some(message_url) = config
            .message_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(Self::Sse {
                client,
                message_url: message_url.to_string(),
                headers: expand_env_map(&config.headers),
                session_id: None,
            });
        }
        // 2. 经典 SSE：GET 握手，从 endpoint 事件拿 message URL
        let mut request = client.get(&url).header("accept", "text/event-stream");
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }
        let response = request.send().await.context("sse handshake")?;
        if !response.status().is_success() {
            bail!("sse handshake status {}", response.status());
        }
        let text = response.text().await.unwrap_or_default();
        let message_url = parse_sse_endpoint(&text, &url)
            .ok_or_else(|| anyhow::anyhow!("sse handshake missing endpoint event"))?;
        Ok(Self::Sse {
            client,
            message_url,
            headers: expand_env_map(&config.headers),
            session_id: None,
        })
    }

    /// 通过当前传输发送 JSON-RPC 请求。
    ///
    /// 参数:
    /// - `id`: JSON-RPC 请求 ID
    /// - `payload`: 完整请求载荷
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - JSON-RPC result 字段
    async fn request(&mut self, id: u64, payload: Value, timeout_ms: Option<u64>) -> Result<Value> {
        match self {
            Self::Stdio { stdin, reader, .. } => {
                let line = format!("{payload}\n");
                stdin.write_all(line.as_bytes()).await?;
                stdin.flush().await?;
                read_stdio_response(reader, id, timeout_ms).await
            }
            Self::Http {
                client,
                url,
                headers,
                session_id,
            } => {
                let mut request = client
                    .post(url.as_str())
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .json(&payload);
                for (key, value) in headers.iter() {
                    request = request.header(key, value);
                }
                if let Some(session) = session_id.as_ref() {
                    request = request.header("mcp-session-id", session);
                }
                let response = request.send().await.context("mcp http request")?;
                if let Some(value) = response
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|value| value.to_str().ok())
                {
                    *session_id = Some(value.to_string());
                }
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    bail!("mcp http status {status}: {body}");
                }
                parse_rpc_body(&body, &content_type, id)
            }
            Self::Sse {
                client,
                message_url,
                headers,
                session_id,
            } => {
                let mut request = client
                    .post(message_url.as_str())
                    .header("content-type", "application/json")
                    .header("accept", "application/json, text/event-stream")
                    .json(&payload);
                for (key, value) in headers.iter() {
                    request = request.header(key, value);
                }
                if let Some(session) = session_id.as_ref() {
                    request = request.header("mcp-session-id", session);
                }
                let response = request.send().await.context("mcp sse message")?;
                if let Some(value) = response
                    .headers()
                    .get("mcp-session-id")
                    .and_then(|value| value.to_str().ok())
                {
                    *session_id = Some(value.to_string());
                }
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let body = response.text().await.unwrap_or_default();
                if !status.is_success() {
                    bail!("mcp sse status {status}: {body}");
                }
                parse_rpc_body(&body, &content_type, id)
            }
        }
    }

    /// 通过当前传输发送 JSON-RPC 通知。
    ///
    /// 参数:
    /// - `payload`: 完整通知载荷
    /// - `timeout_ms`: 可选请求超时时间
    ///
    /// 返回:
    /// - 通知发送结果
    async fn notify(&mut self, payload: Value, timeout_ms: Option<u64>) -> Result<()> {
        let _ = timeout_ms;
        match self {
            Self::Stdio { stdin, .. } => {
                let line = format!("{payload}\n");
                stdin.write_all(line.as_bytes()).await?;
                stdin.flush().await?;
                Ok(())
            }
            Self::Http {
                client,
                url,
                headers,
                session_id,
            }
            | Self::Sse {
                client,
                message_url: url,
                headers,
                session_id,
            } => {
                let mut request = client
                    .post(url.as_str())
                    .header("content-type", "application/json")
                    .json(&payload);
                for (key, value) in headers.iter() {
                    request = request.header(key, value);
                }
                if let Some(session) = session_id.as_ref() {
                    request = request.header("mcp-session-id", session);
                }
                let response = request.send().await.context("mcp notify")?;
                if !response.status().is_success()
                    && response.status().as_u16() != 202
                    && response.status().as_u16() != 204
                {
                    bail!("mcp notify status {}", response.status());
                }
                Ok(())
            }
        }
    }
}

/// 从 stdio 输出中读取与请求 ID 匹配的 JSON-RPC 响应。
///
/// 参数:
/// - `reader`: MCP 子进程标准输出读取器
/// - `id`: 目标 JSON-RPC 请求 ID
/// - `timeout_ms`: 可选请求超时时间
///
/// 返回:
/// - JSON-RPC result 字段
async fn read_stdio_response(
    reader: &mut BufReader<ChildStdout>,
    id: u64,
    timeout_ms: Option<u64>,
) -> Result<Value> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000).clamp(500, 180_000));
    let started = std::time::Instant::now();
    loop {
        if started.elapsed() > timeout {
            bail!("mcp request timed out");
        }
        let mut buf = String::new();
        let read =
            tokio::time::timeout(Duration::from_millis(500), reader.read_line(&mut buf)).await;
        match read {
            Ok(Ok(0)) => bail!("mcp server closed stdout"),
            Ok(Ok(_)) => {
                let text = buf.trim();
                if text.is_empty() {
                    continue;
                }
                let value: Value = serde_json::from_str(text)
                    .with_context(|| format!("invalid mcp json: {text}"))?;
                if matches_id(&value, id) {
                    if let Some(error) = value.get("error") {
                        bail!("mcp error: {error}");
                    }
                    return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                }
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests;
