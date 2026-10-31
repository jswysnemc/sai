use super::{tool_execution::RealToolExecution, Agent, AgentEvent};
use crate::llm::{ChatMessage, ToolCall};
use crate::tools::codemode::{self, output};
use anyhow::{bail, Context, Result};
use sai_codemode::output::Output;
use sai_codemode::protocol::{ExecutionRequest, MAX_CODE_BYTES};
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

impl Agent {
    /// 【CodeMode】【执行入口】运行 turn_id/call 脚本，messages 用于权限审核，on_event 交付进度。
    /// 返回预算内输出、图片和失败状态；失败不提交状态增量。
    pub(super) async fn execute_codemode(
        &mut self,
        turn_id: &str,
        call: &ToolCall,
        messages: &[ChatMessage],
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<RealToolExecution> {
        let args = super::first_json_object(&call.function.arguments)?;
        let code = args["code"].as_str().context("codemode code is required")?;
        if code.len() > MAX_CODE_BYTES {
            bail!("codemode script exceeds size limit");
        }
        self.config.codemode.validate()?;
        let (timeout, budget) = match output::options(code, self.config.codemode.timeout_ms) {
            Ok(options) => options,
            Err(error) => {
                return Ok(RealToolExecution {
                    output: format!("tool error: {error}"),
                    model_attachments: Vec::new(),
                    failed: true,
                })
            }
        };
        let request = ExecutionRequest {
            code: code.into(),
            timeout_ms: timeout,
            catalog: codemode::catalog(&self.tools),
            store: self.state.load_codemode_store(turn_id)?,
        };
        let cancel = self.cancel_requested.clone();
        let mut collected = Output::default();
        let mut pending_approvals = BTreeSet::new();
        let result = {
            let mut forward = |event| {
                match &event {
                    AgentEvent::PermissionRequested(request) => {
                        pending_approvals.insert(request.id.clone());
                    }
                    AgentEvent::PermissionResolved { request_id, .. } => {
                        pending_approvals.remove(request_id);
                    }
                    _ => {}
                }
                on_event(event)
            };
            let execution =
                self.run_codemode_worker(request, messages, &mut collected, &mut forward);
            tokio::pin!(execution);
            // 1. 【CodeMode】【硬截止】超时包含进程启动、工具执行与人工审核等待
            tokio::select! {
                result = &mut execution => result,
                _ = tokio::time::sleep(std::time::Duration::from_millis(timeout)) => Err(anyhow::anyhow!("codemode script timed out")),
                _ = async {
                    while !cancel.load(Ordering::SeqCst) {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    }
                } => Err(anyhow::anyhow!("codemode script cancelled")),
            }
        };
        for request_id in pending_approvals {
            on_event(AgentEvent::PermissionResolved {
                request_id,
                decision: crate::permission::PermissionDecision::Deny {
                    reply: Some("脚本已结束或取消".into()),
                },
            })?;
        }
        let (mut error, writes) = match result {
            Ok(completion) => completion,
            Err(error) => (Some(error.to_string()), Default::default()),
        };
        // 2. 【CodeMode】【成功提交】先验证并渲染图片，再提交状态；输出失败同样不产生状态写入
        let directory = self.state.state_dir().join("tool-results");
        let rendered = output::render(collected, error.as_deref(), &directory, budget);
        let (mut text, attachments) = match rendered {
            Ok((text, attachments, failure)) => {
                error = failure;
                (text, attachments)
            }
            Err(render_error) => {
                error = Some(render_error.to_string());
                (
                    format!("tool error: Script failed\nScript error: {render_error}"),
                    Vec::new(),
                )
            }
        };
        if error.is_none() && !writes.is_empty() {
            if let Err(commit_error) = self.state.commit_codemode_store(turn_id, &call.id, &writes)
            {
                error = Some(commit_error.to_string());
                text = format!("tool error: Script failed\n{text}\nScript error: {commit_error}");
            }
        }
        if !attachments.is_empty() {
            self.state
                .record_tool_result_images(turn_id, &call.id, &attachments)?;
        }
        Ok(RealToolExecution {
            output: text,
            model_attachments: attachments,
            failed: error.is_some(),
        })
    }
}
