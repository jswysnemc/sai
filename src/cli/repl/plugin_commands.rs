use crate::agent::{Agent, AgentMode};
use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::repl_tool_warmup::ReplToolWarmup;
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::ToolRegistry;
use anyhow::{bail, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use serde_json::json;
use std::time::Duration;

/// 【插件终端】【输入分派】处理插件目录、重新加载和会话内命令。
/// @param input 用户输入；agent 为当前会话；runtime 为终端；warmup 为后台发现；paths、config、mode 为宿主配置
/// @returns 是否消费本次输入，错误只进入终端提示
pub(super) async fn handle(
    input: &str,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    warmup: &mut ReplToolWarmup,
    paths: &SaiPaths,
    config: &AppConfig,
    mode: AgentMode,
) -> Result<bool> {
    if let Some(rest) = crate::cli::repl_command_rest(input, "/plugins") {
        let result = match rest.trim() {
            "" => show_catalog(agent, runtime),
            "reload" => reload(agent, runtime, warmup, paths, config, mode),
            _ => Err(anyhow::anyhow!("Usage: /plugins [reload]")),
        };
        if let Err(error) = result {
            runtime.record_meta(format!("{error:#}"))?;
        }
        return Ok(true);
    }
    if let Some(rest) = crate::cli::repl_command_rest(input, "/plugin") {
        agent.apply_live_mode(mode);
        if let Err(error) = run(rest, agent, runtime).await {
            runtime.record_meta(format!("{error:#}"))?;
        }
        return Ok(true);
    }
    Ok(false)
}

/// 【插件终端】【加载诊断】使用既有 transcript 展示错误，不破坏终端受管区域。
/// @param agent 当前会话；runtime 为终端
/// @returns 展示结果
pub(super) fn show_diagnostics(agent: &Agent, runtime: &mut ReplRuntime) -> Result<()> {
    for diagnostic in agent.plugin_diagnostics() {
        runtime.record_meta(format!(
            "{} {}: {}",
            t("Plugin failed", "插件加载失败"),
            diagnostic.source,
            diagnostic.error
        ))?;
    }
    Ok(())
}

/// 【插件终端】【运行目录】展示已加载版本与直接用户命令。
/// @param agent 当前会话；runtime 为终端
/// @returns 展示结果
fn show_catalog(agent: &Agent, runtime: &mut ReplRuntime) -> Result<()> {
    let mut lines = agent
        .active_plugins()
        .into_iter()
        .map(|(id, version)| format!("{id}  {version}"))
        .collect::<Vec<_>>();
    for (id, command) in agent.plugin_commands() {
        lines.push(format!(
            "/plugin {id}/{}  {}",
            command.name, command.description
        ));
    }
    lines.push(
        t(
            "/plugins reload applies installed changes; sai plugins manages packages.",
            "/plugins reload 应用安装变更；sai plugins 管理插件包。",
        )
        .to_string(),
    );
    runtime.record_meta(lines.join("\n"))?;
    show_diagnostics(agent, runtime)
}

/// 【插件终端】【重新加载】共用正常注册入口，重新绑定子任务闭包并保留 Agent 白名单。
/// @param agent 当前会话；runtime 为终端；warmup 为后台发现；paths、config、mode 为当前配置
/// @returns 重新加载结果，不重置模型或当前对话
fn reload(
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    warmup: &mut ReplToolWarmup,
    paths: &SaiPaths,
    config: &AppConfig,
    mode: AgentMode,
) -> Result<()> {
    let registry = crate::cli::build_repl_tool_registry_without_mcp_for_session(
        config,
        paths,
        mode,
        agent.session_id(),
        agent.state().state_dir(),
    )?;
    agent.replace_local_tools(registry)?;
    // 【插件终端】【后台版本】旧预热结果不能在用户重新加载后覆盖新插件快照
    warmup.refresh_if_pending(
        config.clone(),
        paths.clone(),
        mode,
        agent.session_id().to_string(),
        agent.state().state_dir().to_path_buf(),
    );
    runtime.record_meta(t("Plugins reloaded.", "插件已重新加载。").to_string())?;
    show_diagnostics(agent, runtime)
}

/// 【插件终端】【命令执行】命令参数保持原文，权限展示复用项目统一组件。
/// @param input 插件 ID/命令及参数；agent 为当前会话；runtime 为终端
/// @returns 命令执行和终端恢复结果
async fn run(input: &str, agent: &Agent, runtime: &mut ReplRuntime) -> Result<()> {
    let (target, arguments) = input
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((input.trim(), ""));
    let (id, command) = target
        .split_once('/')
        .filter(|(id, command)| !id.is_empty() && !command.is_empty())
        .context("Usage: /plugin <plugin-id>/<command> [arguments]")?;
    let (registry, name) = agent.plugin_command_registry(id, command)?;
    let arguments = json!({"arguments": arguments.trim_start()}).to_string();
    if registry.requires_permission(&name, &arguments)? {
        registry.record_permission_requested(&name, &arguments)?;
        let (request, receiver) =
            crate::permission::request_permission(agent.session_id(), &name, &arguments);
        runtime.record_permission_request(request.clone())?;
        crate::cli::prompt_permission_request_tui(&request, runtime)?;
        let decision = receiver.await?;
        runtime.resolve_permission(&request.id, decision.clone())?;
        match decision {
            crate::permission::PermissionDecision::Allow { .. } => {
                registry.record_permission_approved(&name, &arguments, decision.detail())?
            }
            crate::permission::PermissionDecision::Deny { reply } => {
                registry.record_permission_denied(&name, &arguments, reply.as_deref())?;
                bail!(
                    "{}",
                    reply.unwrap_or_else(|| t("Command denied", "命令已拒绝").to_string())
                );
            }
        }
    }
    let output = execute_live(&registry, &name, &arguments, runtime).await?;
    runtime.record_meta(output)
}

/// 【插件终端】【异步等待】执行命令时消费有界进度，响应窗口变化和取消按键。
/// @param registry 临时执行表；name 为内部命令名；arguments 为 JSON 参数；runtime 为终端
/// @returns 完整命令文本；取消会丢弃 Future，由运行时终止宿主 I/O
async fn execute_live(
    registry: &ToolRegistry,
    name: &str,
    arguments: &str,
    runtime: &mut ReplRuntime,
) -> Result<String> {
    let (sender, mut progress) = tokio::sync::mpsc::unbounded_channel();
    let mut guard =
        crate::cli::terminal_restore::TerminalInputGuard::enable(&mut std::io::stdout(), false)?;
    let result = {
        let call = registry.call_with_progress(name, arguments, sender);
        tokio::pin!(call);
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                result = &mut call => break result.map(|output| output.content),
                Some(message) = progress.recv() => runtime.record_meta(message)?,
                _ = tick.tick() => {
                    runtime.tick_live()?;
                    if cancelled()? { break Err(anyhow::anyhow!(t("Plugin command cancelled", "插件命令已取消"))); }
                },
            }
        }
    };
    guard.finish(&mut std::io::stdout())?;
    result
}

/// 【插件终端】【取消按键】仅消费命令执行期间的按键，Esc 和 Ctrl+C 停止命令。
/// @returns 用户是否请求取消
fn cancelled() -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Release
                && (key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
