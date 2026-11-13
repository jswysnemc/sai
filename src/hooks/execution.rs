//! 有界命令输入输出、完整超时和进程组取消，HTTP 是 Sai 保留的扩展动作。

use super::{protocol, HookContext, HookEvent};
use crate::config::HookItem;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

struct ProcessGuard {
    pid: Option<u32>,
}
impl Drop for ProcessGuard {
    /// 【Hooks】【进程清理】取消或超时时终止进程树，无参数和返回值
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
    }
}

/// 【Hooks】【执行时限】参数为配置与事件，返回完整执行时限
pub(super) fn timeout(hook: &HookItem, event: HookEvent) -> Duration {
    let shutdown = matches!(event, HookEvent::SessionEnd | HookEvent::Interrupt);
    Duration::from_millis(
        hook.timeout_ms
            .unwrap_or(if shutdown { 1000 } else { 600_000 })
            .clamp(100, if shutdown { 3000 } else { 600_000 }),
    )
}

/// 【Hooks】【命令执行】参数为配置、事件与上下文，返回结构化输出及退出二反馈
pub(super) async fn command(
    hook: &HookItem,
    event: HookEvent,
    context: &HookContext,
) -> Result<(Value, Option<String>)> {
    let script = if cfg!(windows) {
        hook.options
            .command_windows
            .as_deref()
            .unwrap_or(&hook.script)
    } else {
        &hook.script
    };
    if script.trim().is_empty() {
        return Ok((serde_json::json!({}), None));
    }
    let invocation = crate::platform::shell::configured_command_invocation(
        hook.options.shell.as_deref(),
        script,
    );
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !context.workdir.is_empty() {
        command.current_dir(&context.workdir);
    }
    if let Some(source) = &hook.options.runtime {
        command.env_clear().envs(&source.environment);
        command
            .env("SAI_HOOK_ROOT", &source.root)
            .env("SAI_HOOK_DATA_DIR", &source.output_dir);
    }
    command
        .env("SAI_HOOK_EVENT", event.as_str())
        .env("SAI_HOOK_NAME", &hook.name)
        .env("SAI_SESSION_ID", &context.session_id)
        .env("SAI_WORKDIR", &context.workdir);
    if let Some(tool) = &context.tool_name {
        command.env("SAI_TOOL_NAME", tool);
    }
    for (key, value) in &context.extra {
        if key.starts_with("SAI_") {
            command.env(key, value);
        }
    }
    crate::platform::job_control::isolate_tokio_command(&mut command);
    let mut child = command.spawn().context("spawn hook")?;
    let mut guard = ProcessGuard { pid: child.id() };
    let mut stdin = child.stdin.take().context("hook stdin")?;
    let stdout = child.stdout.take().context("hook stdout")?;
    let stderr = child.stderr.take().context("hook stderr")?;
    let input = serde_json::to_vec(&protocol::input(event, context))?;
    let execution = async {
        let write = async {
            let result = stdin.write_all(&input).await;
            // 1. 【Hooks】【退出优先】不读标准输入的阻断脚本可能关闭管道，退出二仍必须生效
            if let Err(error) = result {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(error);
                }
            }
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let read = |stream: std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>>| async move {
            let mut bytes = Vec::new();
            stream.take(1_048_577).read_to_end(&mut bytes).await?;
            if bytes.len() > 1_048_576 {
                bail!("hook output exceeds 1 MiB");
            }
            Ok::<_, anyhow::Error>(bytes)
        };
        let (_, out, err, status) = tokio::try_join!(
            async { write.await.map_err(anyhow::Error::from) },
            read(Box::pin(stdout)),
            read(Box::pin(stderr)),
            async { child.wait().await.map_err(anyhow::Error::from) }
        )?;
        Ok::<_, anyhow::Error>((out, err, status))
    };
    let (out, err, status) = tokio::time::timeout(timeout(hook, event), execution)
        .await
        .context("hook timed out")??;
    guard.pid = None;
    let error = crate::platform::output_encoding::decode_output(&err);
    if status.code() == Some(2) {
        return Ok((
            serde_json::json!({}),
            Some(if error.trim().is_empty() {
                "Blocked by hook".into()
            } else {
                error.trim().into()
            }),
        ));
    }
    if !status.success() {
        bail!("hook exited {:?}: {}", status.code(), error.trim());
    }
    Ok((
        protocol::output(
            &crate::platform::output_encoding::decode_output(&out),
            event,
        )?,
        None,
    ))
}

/// 【Hooks】【HTTP 动作】参数为配置、事件和上下文，返回响应对象列表
pub(super) async fn http(
    hook: &HookItem,
    event: HookEvent,
    context: &HookContext,
) -> Result<Vec<Value>> {
    let client = reqwest::Client::builder()
        .timeout(timeout(hook, event))
        .build()?;
    let mut outputs = Vec::new();
    for request in &hook.requests {
        if request.url.trim().is_empty() {
            continue;
        }
        let method = reqwest::Method::from_bytes(request.method.to_uppercase().as_bytes())?;
        let mut builder = client.request(method.clone(), &request.url);
        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        if method != reqwest::Method::GET {
            builder = builder.header("content-type", "application/json").body(
                if request.body.is_empty() {
                    protocol::input(event, context).to_string()
                } else {
                    request.body.clone()
                },
            );
        }
        let mut response = builder.send().await?.error_for_status()?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 1_048_576 {
                bail!("hook HTTP output exceeds 1 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(bytes)?;
        outputs.push(protocol::output(&text, event)?);
    }
    Ok(outputs)
}
