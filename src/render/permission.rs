use crate::render::terminal_text as t;
use crate::permission::PermissionDecision;

/// 权限选择项索引。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PermissionChoice {
    Allow = 0,
    Deny = 1,
    DenyWithReply = 2,
}

impl PermissionChoice {
    /// 返回所有可选操作。
    pub(crate) fn all() -> [Self; 3] {
        [Self::Allow, Self::Deny, Self::DenyWithReply]
    }

    /// 从索引解析选择项，越界时回退到 Allow。
    pub(crate) fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Deny,
            2 => Self::DenyWithReply,
            _ => Self::Allow,
        }
    }

    /// 返回 0 起始索引。
    pub(crate) fn index(self) -> usize {
        self as usize
    }

    /// 向上移动选择。
    pub(crate) fn prev(self) -> Self {
        Self::from_index(self.index().saturating_sub(1))
    }

    /// 向下移动选择。
    pub(crate) fn next(self) -> Self {
        Self::from_index((self.index() + 1).min(2))
    }

    /// 返回选项标签。
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Allow => t("Allow once", "允许一次"),
            Self::Deny => t("Deny", "拒绝"),
            Self::DenyWithReply => t("Deny and tell Sai how to adjust", "拒绝并告诉 Sai 如何调整"),
        }
    }
}

/// 渲染附着在既有工具视图下方的权限选择。
///
/// 参数:
/// - `selected`: 当前高亮选项
/// - `reply_draft`: 可选拒绝回复草稿
///
/// 返回:
/// - 不重复工具参数的 ANSI 交互文本
pub(crate) fn render_permission_controls(
    selected: PermissionChoice,
    reply_draft: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    for choice in PermissionChoice::all() {
        let active = choice == selected;
        if active {
            lines.push(format!("  \x1b[1;36m❯ {}\x1b[0m", choice.label()));
        } else {
            lines.push(format!("    {}", choice.label()));
        }
    }
    if let Some(draft) = reply_draft {
        lines.push(format!("  \x1b[2m{}\x1b[0m", t("Reply to Sai", "回复 Sai")));
        lines.push(format!("    {draft}\x1b[36m▌\x1b[0m"));
    }
    if reply_draft.is_some() {
        lines.push(format!(
            "  \x1b[2m{}\x1b[0m",
            t("Enter submit · Esc back", "Enter 提交 · Esc 返回")
        ));
    } else {
        lines.push(format!(
            "  \x1b[2m{}\x1b[0m",
            t(
                "Up/Down select · Enter confirm · y allow · n deny",
                "上下键选择 · Enter 确认 · y 允许 · n 拒绝",
            )
        ));
    }
    lines.join("\n")
}

/// 渲染 CLI 审计提示标题行。
///
/// 参数:
/// - `tool`: 待确认的工具名称
/// - `arguments`: 可选工具参数，用于生成对象标签
///
/// 返回:
/// - 标题 ANSI 文本
pub(crate) fn render_permission_title(tool: &str, arguments: Option<&str>) -> String {
    let label = crate::render::tool_event_line::tool_event_label(tool, arguments);
    format!(
        "\x1b[1m{}\x1b[0m  \x1b[2m{label}\x1b[0m",
        t("Permission required", "需要权限确认")
    )
}


/// 渲染自动审核进行中状态行。
///
/// 参数:
/// - `active`: 是否显示（自动审核模式为 true）
///
/// 返回:
/// - ANSI 状态文本；非 active 时为空
pub(crate) fn render_auto_audit_status(active: bool) -> String {
    if !active {
        return String::new();
    }
    format!(
        "  \x1b[2m\x1b[38;5;141m{}\x1b[0m",
        t(
            "Auto audit running · human decision wins if first",
            "自动审核进行中 · 人工先决定则优先生效",
        )
    )
}

/// 渲染附着在既有工具视图下方的权限决定。
///
/// 参数:
/// - `decision`: 用户权限决定
///
/// 返回:
/// - 权限决定 ANSI 文本
pub(crate) fn render_permission_decision(decision: &PermissionDecision) -> String {
    match decision {
        PermissionDecision::Allow => {
            format!("  \x1b[32m{}\x1b[0m", t("Allowed once", "已允许一次"))
        }
        PermissionDecision::Deny { reply } => {
            let mut output = format!("  \x1b[31m{}\x1b[0m", t("Denied", "已拒绝"));
            if let Some(reply) = reply.as_deref().filter(|value| !value.trim().is_empty()) {
                output.push_str(&format!("\n  \x1b[2m{}: \x1b[0m", t("Reply", "回复")));
                output.push_str(reply.trim());
            }
            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证内嵌权限选择不重复绘制工具参数。
    #[test]
    fn permission_controls_do_not_render_tool_content() {
        let output = render_permission_controls(PermissionChoice::Allow, None);

        assert!(output.contains("❯"));
        assert!(output.contains(t("Allow once", "允许一次")));
        assert!(!output.starts_with('\n'));
        assert!(!output.contains(t("Permission required", "需要权限确认")));
        assert!(!output.contains("args:"));
    }

    #[test]
    fn permission_title_includes_tool_label() {
        let output = render_permission_title("edit_file", Some(r#"{"path":"src/main.rs"}"#));
        assert!(output.contains(t("Permission required", "需要权限确认")));
        assert!(output.contains("Edit main.rs"));
        // 标题只展示对象标签，不重复整段参数
        assert!(!output.contains("{\"path\""));
    }

    #[test]
    fn permission_choice_moves_with_wrap_limits() {
        assert_eq!(PermissionChoice::Allow.next(), PermissionChoice::Deny);
        assert_eq!(
            PermissionChoice::DenyWithReply.next(),
            PermissionChoice::DenyWithReply
        );
        assert_eq!(PermissionChoice::Allow.prev(), PermissionChoice::Allow);
        assert_eq!(PermissionChoice::Deny.prev(), PermissionChoice::Allow);
    }
}
