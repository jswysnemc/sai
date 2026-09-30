use super::command_policy::sandbox_escape_reasons;
use crate::i18n::text as t;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 命令获批后的执行环境。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SandboxScopeKind {
    /// 在工作区沙箱内执行
    Sandboxed,
    /// 获批后离开沙箱，拥有完整文件系统与网络
    Escalated,
    /// 沙箱已关闭或平台不支持，直接执行
    Unsandboxed,
    /// 沙箱启用但后端不可用，命令会失败
    Unavailable,
}

/// 附着在权限请求上的沙箱范围，供审批卡与自动审核使用。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct SandboxScope {
    /// 执行环境
    pub(crate) kind: SandboxScopeKind,
    /// 后端名称：bwrap、seatbelt 或 none
    pub(crate) backend: String,
    /// 沙箱内是否有网络
    pub(crate) network: bool,
    /// 需要提升的原因标识
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) reasons: Vec<String>,
    /// 模型填写的提升理由
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) justification: Option<String>,
}

impl SandboxScope {
    /// 【权限】【沙箱范围】为 `run_command` 请求计算获批后的执行环境。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数文本
    ///
    /// 返回:
    /// - 命令类工具返回范围；其余工具为 `None`
    pub(crate) fn for_request(tool: &str, arguments: &str) -> Option<Self> {
        if tool != "run_command" {
            return None;
        }
        let args = serde_json::from_str::<Value>(arguments).unwrap_or(Value::Null);
        let network = crate::sandbox::network_allowed();
        let backend = crate::sandbox::backend_availability();
        let reasons = sandbox_escape_reasons(&args, network);
        // 1. 平台不支持或用户关闭时直接执行
        let kind = if !crate::sandbox::sandbox_requested() {
            SandboxScopeKind::Unsandboxed
        // 2. 需要提升的命令获批后离开沙箱
        } else if !reasons.is_empty() {
            SandboxScopeKind::Escalated
        // 3. 后端探测失败时明确告知会执行失败
        } else if !backend.available {
            SandboxScopeKind::Unavailable
        } else {
            SandboxScopeKind::Sandboxed
        };
        Some(Self {
            kind,
            backend: backend.backend.to_string(),
            network,
            reasons: reasons
                .iter()
                .map(|reason| reason.as_str().to_string())
                .collect(),
            justification: args
                .get("justification")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string),
        })
    }

    /// 返回给自动审核的一段事实说明（英文，避免模型误读）。
    ///
    /// 返回:
    /// - 审核上下文文本
    pub(crate) fn audit_note(&self) -> String {
        let mut note = match self.kind {
            SandboxScopeKind::Sandboxed => format!(
                "Runs inside the {} sandbox: writes limited to the workspace, credentials hidden, network {}.",
                self.backend,
                if self.network { "allowed" } else { "blocked" }
            ),
            SandboxScopeKind::Escalated => "Escalation: if approved, the command runs OUTSIDE the sandbox with full filesystem and network access. Require a clear, in-scope justification.".to_string(),
            SandboxScopeKind::Unsandboxed => "No OS sandbox on this host; if approved, the command runs with the user's full permissions.".to_string(),
            SandboxScopeKind::Unavailable => "Sandbox enabled but its backend is unavailable; the command would fail.".to_string(),
        };
        if !self.reasons.is_empty() {
            note.push_str(&format!(
                " Escalation reasons: {}.",
                self.reasons.join(", ")
            ));
        }
        if let Some(justification) = &self.justification {
            note.push_str(&format!(" Model justification: {justification}"));
        }
        note
    }

    /// 返回审批卡使用的一行说明。
    ///
    /// 返回:
    /// - 本地化说明
    pub(crate) fn summary(&self) -> String {
        let head = match self.kind {
            SandboxScopeKind::Sandboxed => format!(
                "{} {} · {}",
                t("Runs in sandbox", "在沙箱内执行"),
                self.backend,
                if self.network {
                    t("network allowed", "允许网络")
                } else {
                    t("network blocked", "断开网络")
                }
            ),
            SandboxScopeKind::Escalated => t(
                "Leaves the sandbox if approved: full filesystem and network",
                "批准后在沙箱外执行：完整文件系统与网络权限",
            )
            .to_string(),
            SandboxScopeKind::Unsandboxed => t(
                "No sandbox: runs with your full permissions",
                "未启用沙箱：以你的完整权限执行",
            )
            .to_string(),
            SandboxScopeKind::Unavailable => t(
                "Sandbox backend unavailable: the command will fail",
                "沙箱后端不可用：命令会执行失败",
            )
            .to_string(),
        };
        if self.reasons.is_empty() {
            return head;
        }
        let reasons = self
            .reasons
            .iter()
            .map(|reason| reason_label(reason))
            .collect::<Vec<_>>()
            .join("、");
        format!("{head} · {}{reasons}", t("reason: ", "原因："))
    }
}

/// 把原因标识转换为本地化文本。
fn reason_label(reason: &str) -> &'static str {
    match reason {
        "requested" => t("requested by model", "模型申请"),
        "network" => t("network", "联网"),
        "package_manager" => t("package manager", "包管理器"),
        "outside_path" => t("path outside workspace", "工作区外路径"),
        _ => t("other", "其他"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证只有命令工具附带范围，提升原因与理由被记录。
    #[test]
    fn scope_captures_escalation_reasons_and_justification() {
        assert!(SandboxScope::for_request("write_file", "{}").is_none());
        let scope = SandboxScope::for_request(
            "run_command",
            r#"{"command":"curl https://x.dev","sandbox_permissions":"require_escalated","justification":" fetch schema "}"#,
        )
        .unwrap();
        assert!(scope.reasons.contains(&"requested".to_string()));
        assert_eq!(scope.justification.as_deref(), Some("fetch schema"));
        if crate::sandbox::sandbox_requested() {
            assert_eq!(scope.kind, SandboxScopeKind::Escalated);
            assert!(scope.audit_note().contains("OUTSIDE the sandbox"));
        } else {
            assert_eq!(scope.kind, SandboxScopeKind::Unsandboxed);
        }
    }

    /// 验证普通命令在支持的平台上留在沙箱内。
    #[test]
    fn local_command_stays_sandboxed() {
        let scope =
            SandboxScope::for_request("run_command", r#"{"command":"cargo test"}"#).unwrap();
        assert!(scope.reasons.is_empty());
        if !crate::sandbox::sandbox_requested() {
            assert_eq!(scope.kind, SandboxScopeKind::Unsandboxed);
        } else if crate::sandbox::backend_availability().available {
            assert_eq!(scope.kind, SandboxScopeKind::Sandboxed);
            assert!(scope.audit_note().contains("inside the"));
        }
    }

    /// 验证序列化字段稳定，供 Web 解析。
    #[test]
    fn serializes_for_web() {
        let scope = SandboxScope {
            kind: SandboxScopeKind::Escalated,
            backend: "bwrap".into(),
            network: false,
            reasons: vec!["network".into()],
            justification: None,
        };
        let value = serde_json::to_value(&scope).unwrap();
        assert_eq!(value["kind"], "escalated");
        assert_eq!(value["reasons"][0], "network");
        assert!(value.get("justification").is_none());
        assert!(scope.summary().contains(reason_label("network")));
    }
}
