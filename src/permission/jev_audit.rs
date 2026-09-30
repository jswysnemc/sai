use super::{PermissionDecision, PermissionRequest};
use crate::config::{AppConfig, JevAuditConfig};
use crate::jev::audit::{self, AuditFacts, AuditVerdict};
use crate::jev::JevClient;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// 【Jev审核】【运行时】内置 Jev 权限审核所需的客户端与阈值。
#[derive(Clone)]
pub(crate) struct JevAuditRuntime {
    client: Arc<JevClient>,
    settings: JevAuditConfig,
}

impl JevAuditRuntime {
    /// 按配置构造审核运行时。
    ///
    /// 参数:
    /// - `config`: 当前应用配置
    ///
    /// 返回:
    /// - 审核运行时；接入不存在或密钥缺失时报错，由调用方退回人工审核
    pub(crate) fn from_config(config: &AppConfig) -> Result<Self> {
        let connection = config.jev_connection()?;
        let settings = config.jev.audit.clone();
        let client = JevClient::new(&connection, settings.timeout_seconds)?;
        Ok(Self {
            client: Arc::new(client),
            settings,
        })
    }

    /// 【Jev审核】【决定提交】审核实际请求并提交一次性决定。
    ///
    /// 参数:
    /// - `request`: 待审批的权限请求
    /// - `context`: 近期上下文摘要
    /// - `workdir`: 工作目录
    ///
    /// 返回:
    /// - 是否成功提交；弃权、置信不足或人工已处理时返回 false
    pub(crate) async fn run(
        &self,
        request: &PermissionRequest,
        context: &str,
        workdir: &Path,
    ) -> Result<bool> {
        // 1. 组装审核事实，参数保留宿主原文，避免数值精度丢失
        let workdir = workdir.to_string_lossy();
        let sandbox = request
            .sandbox
            .as_ref()
            .map(super::SandboxScope::audit_note);
        let facts = AuditFacts {
            tool: &request.tool,
            arguments_json: &request.arguments,
            context,
            workdir: &workdir,
            policy: crate::prompts::AUTO_AUDIT_SYSTEM_PROMPT,
            sandbox: sandbox.as_deref(),
        };
        // 2. 请求 Jev 并映射为权限决定
        let decision = match audit::review(&self.client, &facts, &self.settings).await? {
            AuditVerdict::Allow(reason) => PermissionDecision::auto_allow_once(Some(reason)),
            AuditVerdict::Deny(reason) => PermissionDecision::Deny {
                reply: Some(reason),
            },
            AuditVerdict::Abstain(_) => return Ok(false),
        };
        // 3. 提交；人工先到时不覆盖
        super::audit_backend::submit(&request.id, decision)
    }
}
