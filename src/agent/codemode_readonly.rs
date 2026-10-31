use super::{Agent, AgentEvent};
use crate::llm::{ToolCall, ToolCallFunction};
use crate::tools::{codemode, ToolOutput, ToolPermission};
use anyhow::Result;
use sai_codemode::protocol::WorkerEvent;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub(super) type ReadCompletion = (u64, ToolCall, Result<ToolOutput>);

impl Agent {
    /// 【CodeMode】【并发资格】校验 event，返回可并发只读调用；交互、授权和写调用保持串行。
    pub(super) fn codemode_readonly_call(&self, event: &WorkerEvent) -> Option<(u64, ToolCall)> {
        let WorkerEvent::Call {
            id,
            name,
            arguments,
        } = event
        else {
            return None;
        };
        if codemode::dispatch::is_gateway(name)
            || matches!(
                name.as_str(),
                "ask_question" | "enter_plan_mode" | "exit_plan_mode"
            )
        {
            return None;
        }
        let arguments = serde_json::to_string(arguments).ok()?;
        if self.tools.validate_arguments(name, &arguments).is_err()
            || self.tools.permission(name).ok()? != ToolPermission::ReadOnly
            || self.tools.requires_permission(name, &arguments).ok()?
        {
            return None;
        }
        Some((
            *id,
            ToolCall {
                id: format!("codemode-nested-{id}"),
                kind: "function".into(),
                function: ToolCallFunction {
                    name: name.clone(),
                    arguments,
                },
            },
        ))
    }

    /// 【CodeMode】【只读调度】把 id/call 加入 jobs，progress 交付摘要；返回无。
    pub(super) fn spawn_codemode_readonly(
        &self,
        id: u64,
        call: ToolCall,
        jobs: &mut JoinSet<ReadCompletion>,
        progress: mpsc::Sender<AgentEvent>,
    ) {
        let registry = self.tools.clone();
        let hooks = self.config.hooks.clone();
        let workdir = crate::runtime_cwd::current_dir().unwrap_or_else(|_| ".".into());
        let context = crate::hooks::HookContext {
            session_id: self.session_id().into(),
            tool_name: Some(call.function.name.clone()),
            workdir: workdir.display().to_string(),
            extra: Default::default(),
        };
        jobs.spawn(crate::runtime_cwd::scope(workdir, async move {
            crate::hooks::dispatch(&hooks, crate::hooks::HookEvent::ToolExecutionStart, &context).await;
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let future = registry.call_with_progress(&call.function.name, &call.function.arguments, sender);
            tokio::pin!(future);
            let result = loop {
                tokio::select! {
                    result = &mut future => break result,
                    Some(message) = receiver.recv() => {
                        let _ = progress.send(AgentEvent::ToolProgress { name: codemode::NAME.into(), message: format!("{}: {message}", call.function.name) }).await;
                    }
                }
            };
            while let Ok(message) = receiver.try_recv() {
                let _ = progress.send(AgentEvent::ToolProgress { name: codemode::NAME.into(), message: format!("{}: {message}", call.function.name) }).await;
            }
            crate::hooks::dispatch(&hooks, crate::hooks::HookEvent::ToolExecutionEnd, &context).await;
            (id, call.clone(), result)
        }));
    }
}
