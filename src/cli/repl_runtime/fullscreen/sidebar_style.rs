//! 侧栏的文字裁剪和操作提示。

use super::sidebar::Sidebar;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{SidebarPalette, fg};
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
    let keys: &[(&str, &str)] = if sidebar.focused {
        &[("Up/Down", "select"), ("Enter", "open"), ("Esc", "return")]
    } else {
        &[("F6", "focus"), ("Enter", "open"), ("Esc", "return")]
    };
    // 1. 【终端侧栏】【分段省略】从末尾整段舍弃，不在按键名称中间截断
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
    let index = sidebar
        .hovered
        .or_else(|| sidebar.focused.then_some(sidebar.focused_row).flatten());
    let action = index
        .and_then(|index| sidebar.document.rows.get(index))
        .and_then(|row| row.action)?;
    Some(match action {
        SidebarAction::Toggle(section) => {
            let collapsed = match section {
                SidebarSection::Agents => sidebar.sections.agents_collapsed,
                SidebarSection::Git => sidebar.sections.git_collapsed,
                SidebarSection::Todo => sidebar.sections.todo_collapsed,
                SidebarSection::Background => sidebar.sections.background_collapsed,
            };
            if collapsed {
                "Click to expand"
            } else {
                "Click to collapse"
            }
        }
        SidebarAction::Panel(PanelAction::Todo) => {
            if sidebar.document.todo_compact {
                "Click to show all tasks"
            } else {
                "Click to show current task"
            }
        }
        SidebarAction::GitFile(_) => "Enter / click to inspect changes",
        SidebarAction::Background(_) => "Enter / click to manage tasks and logs",
        SidebarAction::Panel(PanelAction::Agent(None)) => "Click to view main conversation",
        SidebarAction::Panel(PanelAction::Agent(Some(_))) => "Click to view subagent",
        SidebarAction::Panel(
            PanelAction::QueueToggle | PanelAction::QueueItem(_) | PanelAction::QueueAction(_),
        ) => "Click to manage queued messages",
        _ => return None,
    })
}
