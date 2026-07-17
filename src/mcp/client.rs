use crate::config::McpServerConfig;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

#[path = "naming.rs"]
mod naming;
pub use naming::dynamic_tool_name;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static POOL: OnceLock<Mutex<HashMap<String, Arc<Mutex<PooledClient>>>>> = OnceLock::new();

fn pool() -> &'static Mutex<HashMap<String, Arc<Mutex<PooledClient>>>> {
    POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
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
    async fn connect(config: &McpServerConfig) -> Result<Self> {
        let transport_name = config.transport.trim().to_ascii_lowercase();
        let transport = match transport_name.as_str() {
            "http" => Transport::connect_http(config).await?,
            "sse" => Transport::connect_sse(config).await?,
            _ => Transport::connect_stdio(config).await?,
        };
        let mut client = Self {
            config_fingerprint: fingerprint(config),
            transport,
            initialized: false,
            last_error: None,
        };
        client.initialize(config.timeout_ms).await?;
        Ok(client)
    }

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

    async fn notify(&mut self, method: &str, params: Value, timeout_ms: Option<u64>) -> Result<()> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.transport.notify(payload, timeout_ms).await
    }

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
    async fn connect_stdio(config: &McpServerConfig) -> Result<Self> {
        if config.command.trim().is_empty() {
            bail!("mcp server {} missing command", config.id);
        }
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
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
        for (key, value) in &config.env {
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
            headers: config.headers.clone(),
            session_id: None,
        })
    }

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
                headers: config.headers.clone(),
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
            headers: config.headers.clone(),
            session_id: None,
        })
    }

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

fn matches_id(value: &Value, id: u64) -> bool {
    value.get("id").and_then(|value| value.as_u64()) == Some(id)
        || value.get("id").and_then(|value| value.as_i64()) == Some(id as i64)
}

fn parse_rpc_body(body: &str, content_type: &str, id: u64) -> Result<Value> {
    if content_type.contains("text/event-stream") || body.contains("event:") {
        for chunk in body.split("\n\n") {
            for line in chunk.lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(data) {
                        if matches_id(&value, id) {
                            if let Some(error) = value.get("error") {
                                bail!("mcp error: {error}");
                            }
                            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                        }
                    }
                }
            }
        }
        bail!("sse response missing matching rpc id");
    }
    let value: Value = serde_json::from_str(body).context("invalid mcp json response")?;
    if matches_id(&value, id) {
        if let Some(error) = value.get("error") {
            bail!("mcp error: {error}");
        }
        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
    }
    // 部分 HTTP 实现直接返回 result 对象
    Ok(value)
}

fn parse_sse_endpoint(body: &str, base_url: &str) -> Option<String> {
    let mut event = String::new();
    let mut data = String::new();
    for line in body.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if event == "endpoint" && !data.is_empty() {
                return Some(resolve_url(base_url, data.trim()));
            }
            event.clear();
            data.clear();
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim());
        }
    }
    if event == "endpoint" && !data.is_empty() {
        return Some(resolve_url(base_url, data.trim()));
    }
    None
}

fn resolve_url(base: &str, maybe_relative: &str) -> String {
    if maybe_relative.starts_with("http://") || maybe_relative.starts_with("https://") {
        return maybe_relative.to_string();
    }
    if let Ok(base) = reqwest::Url::parse(base) {
        if let Ok(joined) = base.join(maybe_relative) {
            return joined.to_string();
        }
    }
    maybe_relative.to_string()
}

fn fingerprint(config: &McpServerConfig) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        config.id,
        config.transport,
        config.command,
        config.args.join("\u{1f}"),
        config.url.clone().unwrap_or_default(),
        config.message_url.clone().unwrap_or_default(),
        config.cwd.clone().unwrap_or_default(),
        config.timeout_ms.unwrap_or(0)
    )
}

async fn with_client<T, F, Fut>(config: &McpServerConfig, f: F) -> Result<T>
where
    F: FnOnce(Arc<Mutex<PooledClient>>) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let key = config.id.clone();
    let fp = fingerprint(config);
    let client = {
        let mut map = pool().lock().await;
        if let Some(existing) = map.get(&key) {
            let guard = existing.lock().await;
            if guard.config_fingerprint == fp && guard.initialized {
                drop(guard);
                existing.clone()
            } else {
                drop(guard);
                map.remove(&key);
                let created = Arc::new(Mutex::new(PooledClient::connect(config).await?));
                map.insert(key.clone(), created.clone());
                created
            }
        } else {
            let created = Arc::new(Mutex::new(PooledClient::connect(config).await?));
            map.insert(key, created.clone());
            created
        }
    };
    f(client).await
}

/// 停止连接池中的指定 server。
pub async fn stop_server(server_id: &str) -> bool {
    let mut map = pool().lock().await;
    map.remove(server_id).is_some()
}

/// 清空连接池。
pub async fn stop_all_servers() {
    let mut map = pool().lock().await;
    map.clear();
}

/// 运行态状态列表。
pub async fn runtime_statuses(servers: &[McpServerConfig]) -> Vec<McpRuntimeStatus> {
    let map = pool().lock().await;
    servers
        .iter()
        .map(|server| {
            if let Some(client) = map.get(&server.id) {
                // try_lock to avoid deadlock if held elsewhere
                if let Ok(guard) = client.try_lock() {
                    return McpRuntimeStatus {
                        server_id: server.id.clone(),
                        transport: server.transport.clone(),
                        running: true,
                        initialized: guard.initialized,
                        last_error: guard.last_error.clone(),
                    };
                }
                return McpRuntimeStatus {
                    server_id: server.id.clone(),
                    transport: server.transport.clone(),
                    running: true,
                    initialized: true,
                    last_error: None,
                };
            }
            McpRuntimeStatus {
                server_id: server.id.clone(),
                transport: server.transport.clone(),
                running: false,
                initialized: false,
                last_error: None,
            }
        })
        .collect()
}

/// 列出单个 MCP server 的工具。
pub async fn list_server_tools(config: &McpServerConfig) -> Result<Vec<McpToolInfo>> {
    with_client(config, |client| async move {
        let mut guard = client.lock().await;
        let tools = guard.list_tools(config.timeout_ms).await?;
        Ok(tools
            .into_iter()
            .map(|(name, description, input_schema)| McpToolInfo {
                server_id: config.id.clone(),
                name,
                description,
                input_schema,
            })
            .collect())
    })
    .await
}

/// 调用 MCP 工具。
pub async fn call_server_tool(
    config: &McpServerConfig,
    tool_name: &str,
    arguments: Value,
) -> Result<String> {
    with_client(config, |client| async move {
        let mut guard = client.lock().await;
        guard
            .call_tool(tool_name, arguments, config.timeout_ms)
            .await
    })
    .await
}

/// 测试连接并返回工具数量。
pub async fn test_server(config: &McpServerConfig) -> Result<(usize, Vec<String>)> {
    // 测试时先踢掉旧连接，再重建
    let _ = stop_server(&config.id).await;
    let tools = list_server_tools(config).await?;
    Ok((
        tools.len(),
        tools.into_iter().map(|tool| tool.name).collect(),
    ))
}

/// 汇总全部启用 server 的工具。
pub async fn list_enabled_tools(servers: &[McpServerConfig]) -> Vec<McpToolInfo> {
    let mut all = Vec::new();
    for server in servers.iter().filter(|server| server.enabled) {
        match list_server_tools(server).await {
            Ok(mut tools) => all.append(&mut tools),
            Err(error) => eprintln!("[mcp] list tools for {}: {error}", server.id),
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::{dynamic_tool_name, parse_sse_endpoint};

    #[test]
    fn dynamic_tool_name_is_stable_and_sanitized() {
        let name = dynamic_tool_name("File System", "read/file");
        assert!(name.starts_with("mcp_"));
        assert!(!name.contains('/'));
        assert!(!name.contains(' '));
    }

    #[test]
    fn parse_sse_endpoint_absolute_and_relative() {
        let body = "event: endpoint\ndata: /messages?session=1\n\n";
        let url = parse_sse_endpoint(body, "http://127.0.0.1:3000/sse").unwrap();
        assert_eq!(url, "http://127.0.0.1:3000/messages?session=1");
        let body2 = "event: endpoint\ndata: http://example.com/m\n\n";
        assert_eq!(
            parse_sse_endpoint(body2, "http://127.0.0.1:3000/sse").unwrap(),
            "http://example.com/m"
        );
    }
}
