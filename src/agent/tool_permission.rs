use super::{Agent, AgentEvent, AgentMode};
use crate::llm::{ChatMessage, ToolCall};
use crate::permission::{self, PermissionDecision};
use anyhow::{bail, Result};

struct PendingApproval {
    id: String,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for PendingApproval {
    /// 【权限】【请求清理】无参数；取消等待时移除本请求并停止自动审核任务。
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
        let _ = permission::decide_permission(
            &self.id,
            PermissionDecision::Deny {
                reply: Some("工具调用已结束或取消".into()),
            },
        );
    }
}

impl Agent {
    /// 【权限】【统一审批】审核 call，messages 提供审核上下文，on_event 交付交互；返回批准或拒绝。
    pub(super) async fn authorize_tool_call(
        &self,
        call: &ToolCall,
        messages: &[ChatMessage],
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<()> {
        let name = &call.function.name;
        let arguments = &call.function.arguments;
        if !self.tools.requires_permission(name, arguments)? {
            return Ok(());
        }
        self.tools.record_permission_requested(name, arguments)?;
        // 1. 【权限】【自动审核】沿用当前供应商与会话上下文，失败时保留人工审批
        let auto = if self.mode() == AgentMode::AutoAudit {
            permission::AutoAuditBackend::resolve(&self.config, &self.paths).ok()
        } else {
            None
        };
        let (request, receiver) = permission::request_permission_with_auto_audit(
            self.session_id(),
            name,
            arguments,
            auto.is_some(),
        );
        let task = auto.map(|client| {
            let request = request.clone();
            let context = permission::build_audit_context(messages, 2_500);
            let workdir = crate::runtime_cwd::current_dir().unwrap_or_else(|_| ".".into());
            tokio::spawn(async move {
                if let Err(error) = client.run(&request, &context, &workdir).await {
                    let message = format!("{error:#}");
                    if ![
                        "timed out",
                        "timeout",
                        "no longer pending",
                        "no longer running",
                    ]
                    .iter()
                    .any(|part| message.contains(part))
                    {
                        eprintln!("【权限审核】【人工接管】自动审核失败: {message}");
                    }
                }
            })
        });
        let guard = PendingApproval {
            id: request.id.clone(),
            task,
        };
        on_event(AgentEvent::PermissionRequested(request.clone()))?;
        let decision = receiver.await.unwrap_or(PermissionDecision::Deny {
            reply: Some("权限审核通道已关闭".into()),
        });
        on_event(AgentEvent::PermissionResolved {
            request_id: request.id,
            decision: decision.clone(),
        })?;
        drop(guard);
        // 2. 【权限】【决定落地】统一记录批准与拒绝，注册表执行时仍进行第二次授权
        match decision {
            PermissionDecision::Allow { ref reason, .. } => {
                self.tools
                    .record_permission_approved(name, arguments, reason.as_deref())?;
                Ok(())
            }
            PermissionDecision::Deny { reply } => {
                self.tools
                    .record_permission_denied(name, arguments, reply.as_deref())?;
                bail!(
                    "{}",
                    reply
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "用户拒绝了此工具调用".into())
                )
            }
        }
    }
}
