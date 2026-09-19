use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_PERMISSION_AUDIT_INPUT_BYTES: usize = 131_072;

/// 【权限审核】【输入契约】宿主交付完整工具参数和既有审核规则
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionAuditInput {
    pub tool: String,
    pub arguments: Value,
    pub context: String,
    pub policy: String,
}

/// 【权限审核】【决定契约】不确定时保持请求待人工处理
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAuditDecision {
    Allow,
    Deny,
    Abstain,
}

/// 【权限审核】【结果契约】只接受明确的三态决定和有界说明
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionAuditOutput {
    pub decision: PermissionAuditDecision,
    #[serde(default)]
    pub reason: Option<String>,
}

impl PermissionAuditOutput {
    /// 校验返回说明，拒绝控制序列；无参数，返回验证结果
    pub(crate) fn validate(&self) -> Result<()> {
        if self
            .reason
            .as_ref()
            .is_some_and(|reason| reason.len() > 2048 || reason.chars().any(char::is_control))
        {
            bail!("permission audit reason must be plain text within 2048 bytes");
        }
        Ok(())
    }
}
