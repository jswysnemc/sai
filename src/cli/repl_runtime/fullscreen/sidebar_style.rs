//! 侧栏的文字裁剪和操作提示。

use super::sidebar::Sidebar;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use crate::cli::repl_runtime::panel_mouse::PanelAction;
use crate::cli::repl_text::visible_width;
use crate::render::ansi_clip::clip_ansi_to_width;

/// 【终端侧栏】【列宽限制】只在确实超宽时预留省略号，零列时不输出字符。
/// 参数: text 为 ANSI 文本，width 为可用列数；返回: 宽度受限的文本。
pub(super) fn clip(text: &str, width: usize) -> String {
    if width == 0 {
        String::new()
    } else if visible_width(text) <= width {
        text.to_string()
    } else {
        clip_ansi_to_width(text, width, "…")
    }
}

/// 【终端侧栏】【操作提示】显示当前条目的操作；无条目时显示键盘导航，窄栏按段省略。
/// 参数: sidebar 为侧栏状态，width 为面板净宽；返回: 单行操作说明。
pub(super) fn footer(sidebar: &Sidebar, width: usize) -> String {
    let palette = SidebarPalette::current();
    if let Some(text) = action_hint(sidebar) {
        return format!("{}{}\x1b[0m", fg(palette.muted), clip(text, width));
    }
    let keys: Vec<(&str, &str)> = if sidebar.focused {
        if sidebar.sections.focused_section.is_some() {
            vec![
                ("Up/Down", super::sidebar_i18n::key_select()),
                ("Enter", super::sidebar_i18n::key_open()),
                ("f", super::sidebar_i18n::key_all()),
                ("Esc", super::sidebar_i18n::key_return()),
            ]
        } else {
            vec![
                ("Up/Down", super::sidebar_i18n::key_select()),
                ("Enter", super::sidebar_i18n::key_open()),
                ("f", super::sidebar_i18n::key_focus()),
                ("Esc", super::sidebar_i18n::key_return()),
            ]
        }
    } else {
        // 1. 【终端侧栏】【模态诚实】未获得焦点时仅提示当前实际生效的切入与点击操作，不平铺聚焦态按键
        vec![
            ("F6", super::sidebar_i18n::key_focus()),
            (
                super::sidebar_i18n::key_click(),
                super::sidebar_i18n::key_open(),
            ),
        ]
    };
    // 2. 【终端侧栏】【分段省略】从末尾整段舍弃，不在按键名称中间截断
    let mut line = String::new();
    let mut used = 0;
    for (index, (key, label)) in keys.iter().enumerate() {
        let separator = if index == 0 { "" } else { " · " };
        let size = visible_width(separator) + visible_width(key) + 1 + visible_width(label);
        if used + size > width {
            break;
        }
        line.push_str(&format!(
            "{}{separator}{}{key} {}{label}",
            fg(palette.muted),
            fg(palette.text),
            fg(palette.muted)
        ));
        used += size;
    }
    format!("{line}\x1b[0m")
}

/// 【终端侧栏】【条目说明】悬停或聚焦条目时返回其操作说明，无动作时返回空
/// 参数: sidebar 为侧栏状态；返回: 可选的单句说明
fn action_hint(sidebar: &Sidebar) -> Option<&'static str> {
    use crate::i18n::text as t;
    let index = sidebar
        .hovered
        .or_else(|| sidebar.focused.then_some(sidebar.focused_row).flatten());
    let action = index
        .and_then(|index| sidebar.document.rows.get(index))
        .and_then(|row| row.action)?;
    Some(match action {
        SidebarAction::Session(_) => t(
            "Switch session · background agents continue",
            "切换会话 · 后台智能体继续运行",
        ),
        SidebarAction::Toggle(section) => {
            let collapsed = sidebar.sections.is_collapsed(section);
            let content_len = sidebar
                .document
                .panels
                .iter()
                .find(|panel| panel.section == section)
                .map_or(0, |panel| panel.content.len());
            let min_pinned = usize::from(matches!(
                section,
                SidebarSection::Sessions | SidebarSection::Background
            ));
            // 1. 【终端侧栏】【空态提示】无可收起明细时不输出虚假折叠提示
            if !collapsed && content_len <= min_pinned {
                return (section == SidebarSection::Background).then_some(t(
                    "Use Manage tasks or /ps to inspect",
                    "使用管理任务或 /ps 查看详情",
                ));
            }
            if collapsed {
                super::sidebar_i18n::hint_expand()
            } else {
                super::sidebar_i18n::hint_collapse()
            }
        }
        SidebarAction::Panel(PanelAction::Todo) => {
            if sidebar.document.todo_compact {
                t("Click to show all tasks", "点击展开全部待办")
            } else {
                t("Click to show current task", "点击仅显示当前待办")
            }
        }
        SidebarAction::GitFile(_) => t("Enter / click to inspect changes", "回车或点击查看变更差异"),
        SidebarAction::Background(_) => {
            t("Enter / click to manage tasks and logs", "回车或点击管理任务与日志")
        }
        SidebarAction::Panel(PanelAction::Agent(None)) => {
            t("Click to view main conversation", "点击返回主会话")
        }
        SidebarAction::Panel(PanelAction::Agent(Some(_))) => {
            t("Click to view subagent", "点击查看子智能体")
        }
        SidebarAction::Panel(
            PanelAction::QueueToggle | PanelAction::QueueItem(_) | PanelAction::QueueAction(_),
        ) => t("Click to manage queued messages", "点击管理排队消息"),
        _ => return None,
    })
}
