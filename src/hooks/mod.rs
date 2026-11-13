//! 生命周期钩子协议与调度；旧观察事件保留，新控制事件独立触发。
mod async_tasks;
mod decisions;
mod execution;
mod model_attempt;
mod protocol;
mod source_io;
#[cfg(test)]
mod source_tests;
#[cfg(test)]
mod tests;
pub(crate) use decisions::HookOutcome;
pub(crate) use model_attempt::{model_scope, ModelAttempt};

use crate::config::{HookItem, HooksConfig};
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use std::collections::HashMap;

tokio::task_local! { static IN_HANDLER: (); }

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
    PreModelRequest,
    PostModelRequest,
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    PreCompact,
    PostCompact,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    SubagentStart,
    SubagentStop,
    Stop,
    Interrupt,
}
impl HookEvent {
    /// 【Hooks】【事件名称】无参数，返回兼容协议使用的稳定事件名称
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
            Self::PreModelRequest => "PreModelRequest",
            Self::PostModelRequest => "PostModelRequest",
            Self::PreToolUse => "PreToolUse",
            Self::PermissionRequest => "PermissionRequest",
            Self::PostToolUse => "PostToolUse",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::Stop => "Stop",
            Self::Interrupt => "Interrupt",
        }
    }
    /// 【Hooks】【配置事件】参数为配置名称，返回已知事件，兼容新事件的 snake_case 写法
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let normalized = text.replace('_', "").to_ascii_lowercase();
        [
            Self::AgentStart,
            Self::AgentEnd,
            Self::TurnStart,
            Self::TurnEnd,
            Self::MessageStart,
            Self::MessageEnd,
            Self::ToolExecutionStart,
            Self::ToolExecutionEnd,
            Self::PreModelRequest,
            Self::PostModelRequest,
            Self::PreToolUse,
            Self::PermissionRequest,
            Self::PostToolUse,
            Self::PreCompact,
            Self::PostCompact,
            Self::SessionStart,
            Self::SessionEnd,
            Self::UserPromptSubmit,
            Self::SubagentStart,
            Self::SubagentStop,
            Self::Stop,
            Self::Interrupt,
        ]
        .into_iter()
        .find(|event| event.as_str().replace('_', "").to_ascii_lowercase() == normalized)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub session_id: String,
    pub workdir: String,
    pub tool_name: Option<String>,
    pub extra: HashMap<String, String>,
}
impl HookContext {
    /// 【Hooks】【上下文字段】参数为字段和值，返回增加字段后的上下文，JSON 类型不会丢失
    pub(crate) fn with(mut self, field: &str, value: impl serde::Serialize) -> Self {
        if let Ok(value) = serde_json::to_string(&value) {
            self.extra.insert(field.into(), value);
        }
        self
    }
}

/// 【Hooks】【旧观察事件】参数为配置、事件和上下文，返回无值，保留现有用户配置
pub async fn dispatch(config: &HooksConfig, event: HookEvent, context: &HookContext) {
    let _ = dispatch_with_tools(config, event, context, None).await;
}

/// 【Hooks】【统一调度】参数为配置、事件、上下文与可选 MCP 注册表，返回按完成顺序合并的同步控制结果
pub(crate) fn dispatch_with_tools<'a>(
    config: &'a HooksConfig,
    event: HookEvent,
    context: &'a HookContext,
    tools: Option<&'a ToolRegistry>,
) -> futures_util::future::BoxFuture<'a, HookOutcome> {
    Box::pin(async move {
        let mut outcome = HookOutcome::default();
        if !config.enabled || IN_HANDLER.try_with(|_| ()).is_ok() {
            return outcome;
        }
        let input = protocol::input(event, context);
        let mut pending = futures_util::stream::FuturesUnordered::new();
        for hook in &config.items {
            if !hook.enabled || HookEvent::parse(hook.event.trim()) != Some(event) {
                continue;
            }
            match protocol::matches(hook.options.matcher.as_deref(), event, &input) {
                Ok(false) => continue,
                Err(error) => {
                    eprintln!("【Hooks】【匹配失败】{}: {error}", hook.name);
                    continue;
                }
                Ok(true) => {}
            }
            if hook.options.r#async && event != HookEvent::SessionEnd {
                async_tasks::spawn(hook.clone(), event, context.clone(), tools.cloned());
            } else {
                pending.push(async move { (hook, run(hook, event, context, tools).await) });
            }
        }
        use futures_util::StreamExt;
        while let Some((hook, result)) = pending.next().await {
            match result {
                Ok(result) => outcome.merge(result),
                Err(error) => {
                    if hook
                        .options
                        .runtime
                        .as_ref()
                        .is_some_and(|source| source.required)
                    {
                        outcome.stop = true;
                        outcome.deny = Some(format!("Required hook failed: {error:#}"));
                    }
                    eprintln!("【Hooks】【执行失败】{error}");
                }
            }
        }
        outcome
    })
}

/// 【Hooks】【动作执行】参数为单项配置、事件、上下文和 MCP 注册表，返回事件专用决策
async fn run(
    hook: &HookItem,
    event: HookEvent,
    context: &HookContext,
    tools: Option<&ToolRegistry>,
) -> Result<HookOutcome> {
    let started = std::time::Instant::now();
    history::record(hook, event, context, "started", started, None);
    let mut result = async {
        sources::revalidate(hook)?;
        run_action(hook, event, context, tools).await
    }
    .await;
    if let Ok(outcome) = &mut result {
        history::bound(outcome, hook);
    }
    history::record(
        hook,
        event,
        context,
        if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
        started,
        Some(&result),
    );
    result
}

/// 【Hooks】【动作路由】参数为条目、事件、上下文和注册表，返回协议校验后的原始结果
async fn run_action(
    hook: &HookItem,
    event: HookEvent,
    context: &HookContext,
    tools: Option<&ToolRegistry>,
) -> Result<HookOutcome> {
    let results = match hook.kind.as_str() {
        "command" => {
            let (value, two) = execution::command(hook, event, context).await?;
            return Ok(decisions::interpret(event, value, two));
        }
        "http" => execution::http(hook, event, context).await?,
        "mcp_tool" => {
            if event == HookEvent::SessionEnd {
                bail!("SessionEnd does not support MCP hooks");
            }
            let tools = tools
                .ok_or_else(|| anyhow::anyhow!("MCP hook requires an active tool registry"))?;
            let name = hook
                .options
                .mcp_tool
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("missing MCP hook tool"))?;
            if !name.starts_with("mcp_") || !tools.contains(name) {
                bail!("hook target is not an MCP tool: {name}");
            }
            let args = protocol::expand(
                &hook.options.mcp_arguments,
                &protocol::input(event, context),
            )?;
            let output = tokio::time::timeout(
                execution::timeout(hook, event),
                IN_HANDLER.scope((), tools.call(name, &args.to_string())),
            )
            .await??;
            vec![protocol::output(&output, event)?]
        }
        _ => bail!("unsupported hook action: {}", hook.kind),
    };
    let mut outcome = HookOutcome::default();
    for value in results {
        outcome.merge(decisions::interpret(event, value, None));
    }
    Ok(outcome)
}

/// 【Hooks】【异步送达】参数为会话，返回下一安全请求点可使用的非阻断反馈
pub(crate) fn drain(session: &str) -> Vec<String> {
    async_tasks::drain(session)
}
/// 【Hooks】【会话关闭】参数为会话，取消尚未结束的异步钩子，无返回值
pub(crate) fn close(session: &str) {
    async_tasks::close(session);
}

/// 【Hooks】【配置校验】参数为钩子配置，返回未知事件、无效匹配器或动作类型错误
pub(crate) fn validate(config: &HooksConfig) -> Result<()> {
    for hook in &config.items {
        let event = HookEvent::parse(&hook.event)
            .ok_or_else(|| anyhow::anyhow!("unknown hook event: {}", hook.event))?;
        protocol::matches(
            hook.options.matcher.as_deref(),
            event,
            &serde_json::json!({}),
        )?;
        if !matches!(hook.kind.as_str(), "command" | "http" | "mcp_tool") {
            bail!("unsupported hook kind: {}", hook.kind);
        }
        if hook.options.r#async && hook.kind != "command" {
            bail!("only command hooks support async");
        }
        if hook.kind == "mcp_tool" && hook.options.mcp_tool.as_deref().is_none_or(str::is_empty) {
            bail!("MCP hook tool is empty");
        }
    }
    Ok(())
}
pub(crate) mod history;
pub(crate) mod lifecycle;
pub(crate) mod sources;
pub(crate) mod trust;
