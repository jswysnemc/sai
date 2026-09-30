use crate::i18n::text as t;
use crate::permission::{SandboxScope, SandboxScopeKind};
use serde_json::Value;

/// 【终端】【沙箱范围】渲染审批卡上的沙箱范围说明；提升与不可用用醒目色，其余为弱化色。
///
/// 参数:
/// - `scope`: 权限请求附带的沙箱范围
///
/// 返回:
/// - ANSI 文本，可能包含理由第二行
pub(crate) fn render_sandbox_scope(scope: &SandboxScope) -> String {
    let color = match scope.kind {
        SandboxScopeKind::Escalated | SandboxScopeKind::Unavailable => "\x1b[38;5;214m",
        SandboxScopeKind::Sandboxed | SandboxScopeKind::Unsandboxed => "\x1b[2m",
    };
    let mut text = format!("  {color}{}\x1b[0m", scope.summary());
    if let Some(justification) = &scope.justification {
        text.push_str(&format!(
            "\n  \x1b[2m{}{justification}\x1b[0m",
            t("justification: ", "理由：")
        ));
    }
    text
}

/// 【终端】【沙箱拦截】从命令结果 JSON 中提取 `sandbox_denial` 并渲染为提示行。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON
///
/// 返回:
/// - ANSI 提示行；结果中没有拦截说明时为空
pub(crate) fn render_sandbox_denial(output: &str) -> String {
    let Some(denial) = serde_json::from_str::<Value>(output.trim())
        .ok()
        .and_then(|value| value.get("sandbox_denial").cloned())
    else {
        return String::new();
    };
    let kind = match denial.get("kind").and_then(Value::as_str) {
        Some("network") => t("network", "网络"),
        _ => t("filesystem", "文件系统"),
    };
    let hint = denial
        .get("hint")
        .and_then(Value::as_str)
        .unwrap_or_default();
    format!(
        "  \x1b[38;5;214m{} · {kind}\x1b[0m  \x1b[2m{hint}\x1b[0m",
        t("Blocked by sandbox", "沙箱拦截")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指定类型的范围。
    fn scope(kind: SandboxScopeKind, justification: Option<&str>) -> SandboxScope {
        SandboxScope {
            kind,
            backend: "bwrap".into(),
            network: false,
            reasons: vec!["network".into()],
            justification: justification.map(str::to_string),
        }
    }

    /// 验证提升请求使用醒目色并附带理由行，沙箱内请求使用弱化色。
    #[test]
    fn escalation_is_highlighted_with_justification() {
        let text = render_sandbox_scope(&scope(SandboxScopeKind::Escalated, Some("fetch deps")));
        assert!(text.contains("\x1b[38;5;214m"));
        assert!(text.contains("fetch deps"));
        assert_eq!(text.lines().count(), 2);
        let inside = render_sandbox_scope(&scope(SandboxScopeKind::Sandboxed, None));
        assert!(inside.starts_with("  \x1b[2m"));
        assert_eq!(inside.lines().count(), 1);
    }

    /// 验证只有带拦截说明的结果才渲染提示。
    #[test]
    fn denial_line_requires_marker() {
        let denied =
            r#"{"success":false,"sandbox_denial":{"kind":"network","hint":"rerun escalated"}}"#;
        let text = render_sandbox_denial(denied);
        assert!(text.contains("rerun escalated"));
        assert!(text.contains(t("network", "网络")));
        assert!(render_sandbox_denial(r#"{"success":false}"#).is_empty());
        assert!(render_sandbox_denial("not json").is_empty());
    }
}
