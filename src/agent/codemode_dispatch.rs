use super::{Agent, AgentEvent, AgentMode};
use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};
use crate::tools::{codemode, ToolPermission};
use anyhow::{bail, Result};
use sai_codemode::protocol::WorkerEvent;
use serde_json::Value;

impl Agent {
    /// 【CodeMode】【嵌套执行】校验 event 并复用审批、模式、编辑保护与注册表执行链路。
    /// messages 用于审核，questions 限制交互次数，on_event 交付摘要；返回仅供 VM 使用的结果。
    pub(super) async fn execute_codemode_call(
        &mut self,
        event: WorkerEvent,
        messages: &[ChatMessage],
        questions: &mut usize,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<Value> {
        let (id, name, arguments) = codemode::dispatch::validate(&self.tools, event)?;
        let call = ToolCall {
            id: format!("codemode-nested-{id}"),
            kind: "function".into(),
            function: ToolCallFunction {
                name: name.clone(),
                arguments: arguments.clone(),
            },
        };
        let interactive = matches!(
            name.as_str(),
            "ask_question" | "enter_plan_mode" | "exit_plan_mode"
        );
        if !interactive
            && self.mode() == AgentMode::Plan
            && self.tools.permission_for_call(&name, &arguments)? != ToolPermission::ReadOnly
        {
            bail!("Plan mode blocked non-read-only tool: {name}");
        }
        if matches!(name.as_str(), "str_replace" | "write_file") {
            super::edit_guard::ensure_edit_target_was_read(self, &call)?;
        }
        if name == "ask_question" || name == "exit_plan_mode" {
            *questions += 1;
            if *questions > super::tool_invocation::MAX_QUESTION_ROUNDS_PER_TURN {
                bail!("{}", super::tool_invocation::question_limit_notice());
            }
        }
        if !interactive {
            self.authorize_tool_call(&call, messages, on_event).await?;
        }
        on_event(AgentEvent::ToolProgress {
            name: codemode::NAME.into(),
            message: format!("{name} running"),
        })?;
        let hook = crate::hooks::HookContext {
            session_id: self.session_id().into(),
            workdir: crate::runtime_cwd::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            tool_name: Some(name.clone()),
            extra: Default::default(),
        };
        crate::hooks::dispatch(
            &self.config.hooks,
            crate::hooks::HookEvent::ToolExecutionStart,
            &hook,
        )
        .await;
        // 1. 【CodeMode】【专用交互】问题和计划工具必须进入 Agent 专用流程，不能调用占位处理函数
        let result = if name == "ask_question" {
            self.execute_question(&arguments, on_event)
                .await
                .map(|execution| {
                    if execution.failed {
                        anyhow::bail!("{}", execution.output);
                    }
                    Ok(crate::tools::ToolOutput::text(execution.output)
                        .with_model_attachments(execution.model_attachments))
                })
                .and_then(|result| result)
        } else if matches!(name.as_str(), "enter_plan_mode" | "exit_plan_mode") {
            self.execute_plan_tool(&name, &arguments, on_event)
                .await
                .map(|execution| {
                    if execution.failed {
                        anyhow::bail!("{}", execution.output);
                    }
                    Ok(crate::tools::ToolOutput::text(execution.output)
                        .with_model_attachments(execution.model_attachments))
                })
                .and_then(|result| result)
        } else {
            self.execute_codemode_regular(&call, on_event).await
        };
        crate::hooks::dispatch(
            &self.config.hooks,
            crate::hooks::HookEvent::ToolExecutionEnd,
            &hook,
        )
        .await;
        let result = result?;
        if result.content.starts_with("tool error:") {
            bail!("{}", result.content);
        }
        super::edit_guard::record_successful_reads(self, &call, &result.content)?;
        on_event(AgentEvent::ToolProgress {
            name: codemode::NAME.into(),
            message: format!("{name} completed ({} bytes)", result.content.len()),
        })?;
        // 2. 【CodeMode】【结果隔离】原始结果不追加模型消息，只在脚本显式输出时进入上下文
        codemode::dispatch::result_value(result)
    }

    /// 【CodeMode】【注册表代理】执行 call 并转发进度，on_event 接收摘要；返回未经上下文裁剪的结果。
    async fn execute_codemode_regular(
        &self,
        call: &ToolCall,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<crate::tools::ToolOutput> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let future =
            self.tools
                .call_with_progress(&call.function.name, &call.function.arguments, sender);
        tokio::pin!(future);
        loop {
            tokio::select! {
                result = &mut future => {
                    while let Ok(message) = receiver.try_recv() {
                        on_event(AgentEvent::ToolProgress { name: codemode::NAME.into(), message: format!("{}: {message}", call.function.name) })?;
                    }
                    return result;
                }
                Some(message) = receiver.recv() => {
                    on_event(AgentEvent::ToolProgress { name: codemode::NAME.into(), message: format!("{}: {message}", call.function.name) })?;
                }
            }
        }
    }
}
