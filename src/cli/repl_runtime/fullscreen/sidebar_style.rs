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

/// 【终端侧栏】【操作提示】显示当前条目的操作或键盘导航方式。
/// 参数: sidebar 为侧栏状态，height 为内容高度；返回: 单行操作说明。
pub(super) fn footer(sidebar: &Sidebar, _height: usize) -> String {
    let index = sidebar
        .hovered
        .or_else(|| sidebar.focused.then_some(sidebar.focused_row).flatten());
    let action = index
        .and_then(|index| sidebar.document.rows.get(index))
        .and_then(|row| row.action);
    let text = match action {
        Some(SidebarAction::Toggle(section)) => {
            let collapsed = match section {
                SidebarSection::Agents => sidebar.sections.agents_collapsed,
                SidebarSection::Git => sidebar.sections.git_collapsed,
                SidebarSection::Todo => sidebar.sections.todo_collapsed,
            };
            if collapsed {
                "Click to expand"
            } else {
                "Click to collapse"
            }
            .to_string()
        }
        Some(SidebarAction::Panel(PanelAction::Todo)) => if sidebar.document.todo_compact {
            "Click to show all tasks"
        } else {
            "Click to show current task"
        }
        .to_string(),
        Some(SidebarAction::GitFile(_)) => "Enter / click to inspect changes".into(),
        Some(SidebarAction::Panel(PanelAction::Agent(None))) => {
            "Click to view main conversation".into()
        }
        Some(SidebarAction::Panel(PanelAction::Agent(Some(_)))) => "Click to view subagent".into(),
        Some(SidebarAction::Panel(
            PanelAction::QueueToggle | PanelAction::QueueItem(_) | PanelAction::QueueAction(_),
        )) => "Click to manage queued messages".into(),
        _ if sidebar.focused => "Up/Down select · Enter open · Esc".into(),

        _ => "F6 focus · Enter open · Esc return".into(),
    };
    format!("{}{text}\x1b[0m", fg(SidebarPalette::current().muted))
}
