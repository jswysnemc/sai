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
            Self::Allow => "允许一次",
            Self::Deny => "拒绝",
            Self::DenyWithReply => "拒绝并告诉 Sai 如何调整",
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
        lines.push("  \x1b[2m回复 Sai\x1b[0m".to_string());
        lines.push(format!("    {draft}\x1b[36m▌\x1b[0m"));
    }
    if reply_draft.is_some() {
        lines.push("  \x1b[2mEnter 提交 · Esc 返回\x1b[0m".to_string());
    } else {
        lines.push("  \x1b[2m↑↓ 选择 · Enter 确认 · y 允许 · n 拒绝\x1b[0m".to_string());
    }
    lines.join("\n")
}

/// 渲染 CLI 审计提示标题行。
///
/// 参数:
/// - `tool`: 待确认的工具名称
///
/// 返回:
/// - 标题 ANSI 文本
pub(crate) fn render_permission_title(tool: &str) -> String {
    format!("\x1b[1m需要权限确认\x1b[0m  \x1b[2m{tool}\x1b[0m")
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
        PermissionDecision::Allow => "  \x1b[32m已允许一次\x1b[0m".to_string(),
        PermissionDecision::Deny { reply } => {
            let mut output = "  \x1b[31m已拒绝\x1b[0m".to_string();
            if let Some(reply) = reply.as_deref().filter(|value| !value.trim().is_empty()) {
                output.push_str("\n  \x1b[2m回复: \x1b[0m");
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
        assert!(output.contains("允许一次"));
        assert!(!output.starts_with('\n'));
        assert!(!output.contains("需要权限"));
        assert!(!output.contains("args:"));
    }

    #[test]
    fn permission_title_includes_tool_name() {
        let output = render_permission_title("edit_file");
        assert!(output.contains("需要权限确认"));
        assert!(output.contains("edit_file"));
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
