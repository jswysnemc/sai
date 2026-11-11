use super::{deepseek_anchor, Agent};
use crate::llm::{ToolCall, ToolCallFunction};
use crate::tools::codemode;
use anyhow::{bail, Result};
use sai_codemode::protocol::WorkerEvent;
use std::sync::atomic::Ordering;

impl Agent {
    /// 【CodeMode】【统一门禁】参数 event 为脚本调用；校验实时状态、共享可见性和参数，返回真实调用。
    pub(super) fn prepare_codemode_call(&self, event: &WorkerEvent) -> Result<(u64, ToolCall)> {
        let WorkerEvent::Call {
            id,
            name,
            arguments,
        } = event
        else {
            bail!("expected codemode tool call");
        };
        if !self.mode().uses_codemode() || self.cancel_requested.load(Ordering::SeqCst) {
            bail!("codemode is disabled or the current turn was cancelled");
        }
        if let Some(goal) = self.stopped_goal()? {
            bail!("{}", crate::goal::stop_prompt(&goal));
        }
        if codemode::dispatch::is_gateway(name) {
            bail!("codemode cannot recursively call execution or discovery gateways");
        }
        if !self.tool_visibility.is_script_tool_exposed(name) {
            bail!("tool {name} is not exposed; use native load or request_capability before starting a script");
        }
        let mut call = ToolCall {
            id: format!("codemode-nested-{id}"),
            kind: "function".into(),
            function: ToolCallFunction {
                name: name.clone(),
                arguments: serde_json::to_string(arguments)?,
            },
        };
        // 1. 【CodeMode】【名称适配】原生锚定声明与脚本共用同一份参数转换及校验
        if self.tool_visibility.is_anchor_enabled() && deepseek_anchor::is_provider_tool(name) {
            call = deepseek_anchor::resolve_execution_call(&call)?;
        }
        self.tools
            .validate_arguments(&call.function.name, &call.function.arguments)?;
        Ok((*id, call))
    }
}
