use crate::permission::PermissionDecision;
use crate::render::terminal_text as t;

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

/// 权限决定的呈现场景。
///
/// CLI 与 TUI 的排布方式不同，同一份结论要用各自的形态呈现：
/// CLI 是线性输出流，决定独立成行；TUI 里决定附着在工具视图下方，需要缩进对齐。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PermissionView {
    /// CLI 线性输出
    Cli,
    /// TUI 内嵌于工具视图
    Tui,
}

/// 渲染权限决定。
///
/// 人工与自动审核用不同色相区分：人工是当场做的选择，用常规的绿色确认；
/// 自动审核无人值守，用与「自动审核进行中」一致的紫色，事后回看时能一眼分辨
/// 这条放行不是自己点的。理由与回复始终另起一行，不挤在结论后面。
///
/// 参数:
/// - `decision`: 权限决定
/// - `view`: 呈现场景
///
/// 返回:
/// - 权限决定 ANSI 文本
pub(crate) fn render_permission_decision_for(
    decision: &PermissionDecision,
    view: PermissionView,
) -> String {
    // 1. CLI 用项目符号起头独立成行，TUI 缩进两格附着在工具视图下
    let (prefix, detail_prefix) = match view {
        PermissionView::Cli => ("• ", "  "),
        PermissionView::Tui => ("  ", "  "),
    };
    // 2. 结论行：人工绿色、自动紫色、拒绝红色
    let conclusion = match decision {
        PermissionDecision::Allow {
            source: crate::permission::PermissionAllowSource::AutoAudit,
            ..
        } => format!(
            "\x1b[38;5;141m{}\x1b[0m",
            t("Auto-allowed once", "已自动允许一次")
        ),
        PermissionDecision::Allow { .. } => {
            format!("\x1b[32m{}\x1b[0m", t("Allowed once", "已允许一次"))
        }
        PermissionDecision::Deny { .. } => format!("\x1b[31m{}\x1b[0m", t("Denied", "已拒绝")),
    };
    let mut output = format!("{prefix}{conclusion}");
    // 3. 说明行：允许时是自动审核的放行理由，拒绝时是回复给模型的原因
    if let Some(detail) = decision.detail() {
        let label = match decision {
            PermissionDecision::Allow { .. } => t("Reason", "理由"),
            PermissionDecision::Deny { .. } => t("Reply", "回复"),
        };
        output.push_str(&format!("\n{detail_prefix}\x1b[2m{label}: {detail}\x1b[0m"));
    }
    output
}

/// 渲染附着在既有工具视图下方的权限决定。
///
/// 参数:
/// - `decision`: 用户权限决定
///
/// 返回:
/// - 权限决定 ANSI 文本
pub(crate) fn render_permission_decision(decision: &PermissionDecision) -> String {
    render_permission_decision_for(decision, PermissionView::Tui)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证人工与自动放行用不同色相，事后回看能分辨来源。
    #[test]
    fn human_and_auto_approval_use_distinct_colors() {
        let human = render_permission_decision_for(
            &PermissionDecision::allow_once(),
            PermissionView::Tui,
        );
        let auto = render_permission_decision_for(
            &PermissionDecision::auto_allow_once(Some("只读查询".to_string())),
            PermissionView::Tui,
        );

        assert!(human.contains("\x1b[32m"));
        assert!(auto.contains("\x1b[38;5;141m"));
        assert!(!auto.contains("\x1b[32m"));
    }

    /// 验证自动审核的理由在两种视图下都会展示。
    #[test]
    fn auto_audit_reason_shows_in_both_views() {
        let decision = PermissionDecision::auto_allow_once(Some("无副作用".to_string()));
        for view in [PermissionView::Cli, PermissionView::Tui] {
            let output = render_permission_decision_for(&decision, view);
            assert!(output.contains("无副作用"), "{view:?} 未展示自动审核理由");
            assert!(output.contains(t("Reason", "理由")));
        }
    }

    /// 验证 CLI 与 TUI 的行首形态不同。
    #[test]
    fn cli_and_tui_use_different_line_prefixes() {
        let decision = PermissionDecision::allow_once();
        let cli = render_permission_decision_for(&decision, PermissionView::Cli);
        let tui = render_permission_decision_for(&decision, PermissionView::Tui);

        assert!(cli.starts_with("• "));
        assert!(tui.starts_with("  "));
    }

    /// 验证拒绝回复标注为「回复」而非「理由」。
    #[test]
    fn denial_reply_uses_reply_label() {
        let decision = PermissionDecision::Deny {
            reply: Some("风险过高".to_string()),
        };
        let output = render_permission_decision_for(&decision, PermissionView::Cli);

        assert!(output.contains(t("Reply", "回复")));
        assert!(output.contains("风险过高"));
    }

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
        assert!(output.contains("Edit"), "unexpected title: {output}");
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
