//! 主会话生命周期与输入准备，后台反馈只在下一安全模型请求点送达。
use super::{Agent, AgentEvent};
use crate::hooks::{HookContext, HookEvent};
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

impl Agent {
    /// 【Hooks】【专用结果】参数为已准备调用与专用工具结果，返回经过后置反馈的执行状态
    pub(super) async fn finish_special_hook(
        &self,
        prepared: &crate::tools::PreparedToolCall,
        execution: Result<super::tool_execution::RealToolExecution>,
    ) -> super::tool_execution::RealToolExecution {
        let result = execution.and_then(|execution| {
            if execution.failed {
                anyhow::bail!("{}", execution.output);
            }
            Ok(crate::tools::ToolOutput {
                content: execution.output,
                model_attachments: execution.model_attachments,
            })
        });
        super::tool_execution::tool_result(self.tools.finish_hooked_output(prepared, result).await)
    }

    /// 【Hooks】【来源变更】参数为会话来源，返回钩子是否允许继续，并暂存补充上下文
    pub(crate) async fn notify_hook_source(&mut self, source: &str) -> Result<()> {
        let result = crate::hooks::dispatch_with_tools(
            &self.config.hooks,
            HookEvent::SessionStart,
            &self.tools.hook_context().with("source", source),
            Some(&self.tools),
        )
        .await;
        if result.stop {
            anyhow::bail!("SessionStart hook stopped continuation");
        }
        self.pending_hook_context.extend(result.context);
        crate::hooks::lifecycle::register(
            self.config.hooks.clone(),
            self.tools.hook_context(),
            self.tools.clone(),
        );
        Ok(())
    }

    /// 【Hooks】【输入准备】参数为用户输入、轮次和事件接收器，返回补充上下文，拒绝时不发送模型
    pub(super) async fn prepare_input_hooks(
        &self,
        input: &str,
        turn: &str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let context = self.tools.hook_context().with("turn_id", turn).with(
            "model",
            super::model_context::current_model_id(&self.config),
        );
        let mut extra = crate::hooks::drain(self.session_id());
        if !crate::hooks::lifecycle::started(self.session_id()) {
            let source = if self.state.load_turns()?.is_empty() {
                "startup"
            } else {
                "resume"
            };
            let result = crate::hooks::dispatch_with_tools(
                &self.config.hooks,
                HookEvent::SessionStart,
                &context.clone().with("source", source),
                Some(&self.tools),
            )
            .await;
            if result.stop {
                anyhow::bail!("SessionStart hook stopped the session");
            }
            crate::hooks::lifecycle::register(
                self.config.hooks.clone(),
                self.tools.hook_context(),
                self.tools.clone(),
            );
            extra.extend(result.context);
            for message in result.messages {
                on_event(AgentEvent::ToolProgress {
                    name: "hooks".into(),
                    message,
                })?;
            }
        }
        let result = crate::hooks::dispatch_with_tools(
            &self.config.hooks,
            HookEvent::UserPromptSubmit,
            &context.with("prompt", input),
            Some(&self.tools),
        )
        .await;
        if result.stop || result.deny.is_some() {
            anyhow::bail!(
                "UserPromptSubmit: {}",
                result
                    .deny
                    .unwrap_or_else(|| "Input blocked by hook".into())
            );
        }
        extra.extend(result.context);
        crate::hooks::lifecycle::register(
            self.config.hooks.clone(),
            self.tools.hook_context(),
            self.tools.clone(),
        );
        for message in result.messages {
            on_event(AgentEvent::ToolProgress {
                name: "hooks".into(),
                message,
            })?;
        }
        Ok(extra.join("\n"))
    }
    /// 【Hooks】【会话结束】无参数，执行同步收尾并取消后台钩子
    pub(crate) async fn finish_hooks(&self) {
        crate::hooks::lifecycle::finish(self.session_id()).await;
    }
}

pub(super) struct InterruptGuard {
    cancel: Arc<AtomicBool>,
    config: crate::config::HooksConfig,
    context: HookContext,
    tools: crate::tools::ToolRegistry,
}
impl InterruptGuard {
    /// 【Hooks】【中断保护】参数为活动 Agent 与轮次，返回仅在用户取消时触发的保护器
    pub(super) fn new(agent: &Agent, turn: &str) -> Self {
        crate::hooks::lifecycle::activity(agent.session_id(), true);
        Self {
            cancel: agent.cancel_requested.clone(),
            config: agent.config.hooks.clone(),
            context: agent.tools.hook_context().with("turn_id", turn),
            tools: agent.tools.clone(),
        }
    }
}
impl Drop for InterruptGuard {
    /// 【Hooks】【用户中断】活动轮次结束且取消标志置位时报告中断，无参数和返回值
    fn drop(&mut self) {
        crate::hooks::lifecycle::activity(&self.context.session_id, false);
        if !self.cancel.load(Ordering::SeqCst) {
            return;
        }
        let config = self.config.clone();
        let context = self.context.clone();
        let tools = self.tools.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                crate::hooks::dispatch_with_tools(
                    &config,
                    HookEvent::Interrupt,
                    &context,
                    Some(&tools),
                )
                .await;
            });
        }
    }
}
