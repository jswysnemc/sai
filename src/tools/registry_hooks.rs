//! 工具、子代理和模型共享当前注册表的会话身份。
use super::{ToolOutput, ToolRegistry};
use crate::llm::ToolCall;
use anyhow::{bail, Result};

#[derive(Clone)]
pub(crate) struct PreparedToolCall {
    pub(crate) call: ToolCall,
    pub(super) context: crate::hooks::HookContext,
    pub(super) before: crate::hooks::HookOutcome,
}

impl ToolRegistry {
    /// 【Hooks】【前置准备】参数为原始工具调用，返回已执行前置检查的最终调用，审批必须使用该参数
    pub(crate) async fn prepare_hooked_call(&self, call: &ToolCall) -> Result<PreparedToolCall> {
        let mut call = call.clone();
        let mut args = super::parse_arguments(&call.function.arguments)?;
        if let Some(object) = args.as_object_mut() {
            object.retain(|key, _| !key.starts_with("_sai_"));
        }
        let mut context = self
            .hook_context()
            .with("tool_input", &args)
            .with("tool_use_id", &call.id);
        context.tool_name = Some(call.function.name.clone());
        let before = crate::hooks::dispatch_with_tools(
            &self.hooks,
            crate::hooks::HookEvent::PreToolUse,
            &context,
            Some(self),
        )
        .await;
        if let Some(reason) = &before.deny {
            bail!("PreToolUse: {reason}");
        }
        if let Some(updated) = &before.updated_input {
            args = updated.clone();
        }
        if let Some(object) = args.as_object_mut() {
            object.retain(|key, _| !key.starts_with("_sai_"));
        }
        call.function.arguments = args.to_string();
        self.validate_arguments(&call.function.name, &call.function.arguments)?;
        context = context.with("tool_input", &args);
        Ok(PreparedToolCall {
            call,
            context,
            before,
        })
    }

    /// 【Hooks】【后置反馈】参数为已准备调用和实际工具结果，返回保留附件及钩子反馈的模型结果
    pub(crate) async fn finish_hooked_output(
        &self,
        prepared: &PreparedToolCall,
        result: Result<ToolOutput>,
    ) -> Result<ToolOutput> {
        let after = crate::hooks::dispatch_with_tools(
            &self.hooks,
            crate::hooks::HookEvent::PostToolUse,
            &prepared.context.clone().with(
                "tool_response",
                result
                    .as_ref()
                    .map(|output| output.content.as_str())
                    .unwrap_or("tool failed"),
            ),
            Some(self),
        )
        .await;
        if let Some(reason) = after.deny {
            bail!("PostToolUse: {reason}");
        }
        let mut output = result?;
        let extra = prepared
            .before
            .context
            .iter()
            .chain(&after.context)
            .chain(&prepared.before.messages)
            .chain(&after.messages)
            .cloned()
            .collect::<Vec<_>>();
        if !extra.is_empty() {
            output.content.push_str(&format!(
                "\n<hook-context>\n{}\n</hook-context>",
                extra.join("\n")
            ));
        }
        Ok(output)
    }

    /// 【Hooks】【控制优化】无参数，返回是否需要在审批前执行可能改写参数的同步前置钩子
    pub(crate) fn has_control_pre_hooks(&self) -> bool {
        self.hooks.enabled
            && self.hooks.items.iter().any(|hook| {
                hook.enabled
                    && !hook.options.r#async
                    && crate::hooks::HookEvent::parse(&hook.event)
                        == Some(crate::hooks::HookEvent::PreToolUse)
            })
    }

    /// 【Hooks】【注册表上下文】无参数，返回实际会话、工作目录和记录路径
    pub(crate) fn hook_context(&self) -> crate::hooks::HookContext {
        crate::hooks::HookContext {
            session_id: self.session_id.clone(),
            workdir: crate::runtime_cwd::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            ..Default::default()
        }
        .with(
            "transcript_path",
            if self.session_key.is_empty() {
                None
            } else {
                Some(self.session_key.clone())
            },
        )
        .with("agent_id", self.plugin_session_id.clone())
    }
}
