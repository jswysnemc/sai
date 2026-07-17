use crate::config::{HookHttpRequest, HookItem, HooksConfig};
use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

/// Hook 生命周期事件（对齐 LiveAgent 语义的精简子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    AgentStart,
    AgentEnd,
    TurnStart,
    TurnEnd,
    MessageStart,
    MessageEnd,
    ToolExecutionStart,
    ToolExecutionEnd,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentStart => "agent_start",
            Self::AgentEnd => "agent_end",
            Self::TurnStart => "turn_start",
            Self::TurnEnd => "turn_end",
            Self::MessageStart => "message_start",
            Self::MessageEnd => "message_end",
            Self::ToolExecutionStart => "tool_execution_start",
            Self::ToolExecutionEnd => "tool_execution_end",
        }
    }
}

/// 运行上下文。
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub session_id: String,
    pub workdir: String,
    pub tool_name: Option<String>,
    pub extra: HashMap<String, String>,
}

/// 触发匹配事件的 hooks；失败只记警告，不中断主流程。
pub async fn dispatch(config: &HooksConfig, event: HookEvent, context: &HookContext) {
    if !config.enabled {
        return;
    }
    let event_name = event.as_str();
    for hook in &config.items {
        if !hook.enabled || hook.event.trim() != event_name {
            continue;
        }
        if let Err(error) = run_hook(hook, event, context).await {
            eprintln!("[hooks] {} ({event_name}): {error}", hook.name);
        }
    }
}

async fn run_hook(hook: &HookItem, event: HookEvent, context: &HookContext) -> Result<()> {
    match hook.kind.trim().to_ascii_lowercase().as_str() {
        "http" => run_http_hook(hook, event, context).await,
        _ => run_command_hook(hook, event, context).await,
    }
}

async fn run_command_hook(hook: &HookItem, event: HookEvent, context: &HookContext) -> Result<()> {
    let script = hook.script.trim();
    if script.is_empty() {
        return Ok(());
    }
    let timeout = Duration::from_millis(hook.timeout_ms.unwrap_or(30_000).clamp(100, 120_000));
    let mut command = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(script);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-lc").arg(script);
        cmd
    };
    if !context.workdir.trim().is_empty() {
        command.current_dir(&context.workdir);
    }
    command
        .env("SAI_HOOK_EVENT", event.as_str())
        .env("SAI_HOOK_NAME", &hook.name)
        .env("SAI_SESSION_ID", &context.session_id)
        .env("SAI_WORKDIR", &context.workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(tool) = &context.tool_name {
        command.env("SAI_TOOL_NAME", tool);
    }
    for (key, value) in &context.extra {
        if key.starts_with("SAI_") {
            command.env(key, value);
        }
    }
    let child = command.spawn().context("spawn hook command")?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .context("hook command timed out")?
        .context("wait hook command")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("exit {:?}: {}", output.status.code(), stderr.trim());
    }
    Ok(())
}

async fn run_http_hook(hook: &HookItem, event: HookEvent, context: &HookContext) -> Result<()> {
    if hook.requests.is_empty() {
        return Ok(());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(
            hook.timeout_ms.unwrap_or(15_000).clamp(100, 120_000),
        ))
        .build()?;
    for request in &hook.requests {
        send_http(&client, request, event, context, &hook.name).await?;
    }
    Ok(())
}

async fn send_http(
    client: &reqwest::Client,
    request: &HookHttpRequest,
    event: HookEvent,
    context: &HookContext,
    hook_name: &str,
) -> Result<()> {
    let method = request.method.trim().to_ascii_uppercase();
    let url = request.url.trim();
    if url.is_empty() {
        return Ok(());
    }
    let body = if request.body.trim().is_empty() {
        json!({
            "event": event.as_str(),
            "hook": hook_name,
            "session_id": context.session_id,
            "workdir": context.workdir,
            "tool_name": context.tool_name,
        })
        .to_string()
    } else {
        request.body.clone()
    };
    let mut builder = match method.as_str() {
        "GET" => client.get(url),
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        "DELETE" => client.delete(url),
        _ => client.post(url),
    };
    for (key, value) in &request.headers {
        builder = builder.header(key, value);
    }
    if method != "GET" {
        builder = builder
            .header("content-type", "application/json")
            .body(body);
    }
    let response = builder.send().await.context("hook http request")?;
    if !response.status().is_success() {
        anyhow::bail!("http status {}", response.status());
    }
    Ok(())
}
