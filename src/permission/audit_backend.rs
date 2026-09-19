use super::{decide_permission, PermissionDecision, PermissionRequest};
use crate::{config::AppConfig, llm::OpenAiCompatibleClient, paths::SaiPaths};
use anyhow::Result;
use sai_plugin_runtime::{
    InvocationContext, PermissionAuditDecision, PermissionAuditInput, PluginRuntime,
};
use std::path::Path;
use std::time::Duration;

/// 【权限审核】【后端选择】指定 Lua 插件时独占自动审核，不再调用聊天模型
#[derive(Clone)]
pub(crate) enum AutoAuditBackend {
    Llm(OpenAiCompatibleClient),
    Plugin(PluginRuntime),
}

impl AutoAuditBackend {
    /// 按配置选择审核后端，config/paths 为运行配置及目录；返回可执行后端
    pub(crate) fn resolve(config: &AppConfig, paths: &SaiPaths) -> Result<Self> {
        if config.permission.auto_audit_plugin_id.trim().is_empty() {
            Ok(Self::Llm(super::auto_audit::resolve_auto_audit_client(
                config, paths,
            )?))
        } else {
            Ok(Self::Plugin(crate::plugins::permission_audit::load(
                config, paths,
            )?))
        }
    }

    /// 审核实际请求并提交一次性决定；request/context/workdir 为请求、上下文和工作目录
    /// 返回是否成功提交；放弃判断、人工已处理时返回 false
    pub(crate) async fn run(
        &self,
        request: &PermissionRequest,
        context: &str,
        workdir: &Path,
    ) -> Result<bool> {
        match self {
            Self::Llm(client) => {
                super::auto_audit::run_auto_audit(
                    client,
                    &request.id,
                    &request.tool,
                    &request.arguments,
                    context,
                )
                .await
            }
            Self::Plugin(runtime) => {
                let input = PermissionAuditInput {
                    tool: request.tool.clone(),
                    arguments: serde_json::from_str(&request.arguments)?,
                    context: context.into(),
                    policy: crate::prompts::AUTO_AUDIT_SYSTEM_PROMPT.into(),
                };
                let invocation = InvocationContext {
                    session_id: request.session_id.clone(),
                    operation_id: request.id.clone(),
                    workdir: workdir.to_string_lossy().into_owned(),
                    ..Default::default()
                };
                let output = tokio::time::timeout(
                    Duration::from_secs(45),
                    runtime.review_permission(input, invocation),
                )
                .await??;
                let decision = match output.decision {
                    PermissionAuditDecision::Allow => {
                        PermissionDecision::auto_allow_once(output.reason)
                    }
                    PermissionAuditDecision::Deny => PermissionDecision::Deny {
                        reply: output.reason,
                    },
                    PermissionAuditDecision::Abstain => return Ok(false),
                };
                submit(&request.id, decision)
            }
        }
    }
}

/// 【权限审核】【决定提交】保留人工先到的结果；request_id 为请求，decision 为自动决定
/// 返回是否提交；已经处理或终止的请求不会被旧结果覆盖
pub(super) fn submit(request_id: &str, decision: PermissionDecision) -> Result<bool> {
    match decide_permission(request_id, decision) {
        Ok(()) => Ok(true),
        Err(error) => {
            let message = error.to_string();
            if message.contains("no longer pending") || message.contains("no longer running") {
                Ok(false)
            } else {
                Err(error)
            }
        }
    }
}
