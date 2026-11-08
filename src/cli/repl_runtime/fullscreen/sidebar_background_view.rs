//! 后台命令概览使用现有侧栏组件，管理入口在空列表和折叠状态下保持可见。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::SidebarPalette;
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading, item, safe_text};
use crate::cli::repl_background::EmbeddedTaskPanel;

/// 【终端侧栏】【后台概览】显示运行数量、紧凑命令和管理入口，任务按快照顺序独立滚动。
/// 参数: panel 为任务快照，collapsed 为折叠状态，width 为净宽；返回: 带操作的侧栏行。
pub(super) fn rows(panel: &EmbeddedTaskPanel, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let tasks = panel.tasks();
    let running = tasks.iter().filter(|task| task.status == "running").count();
    let mut rows = vec![
        row(
            heading(
                "Background tasks",
                &format!("{running}/{}", tasks.len()),
                collapsed,
                width,
            ),
            Some(SidebarAction::Toggle(SidebarSection::Background)),
        ),
        row(
            item(
                &columns("Manage tasks", &badge("/ps", palette.muted), width),
                width,
                false,
            ),
            Some(SidebarAction::Background(None)),
        ),
    ];
    if collapsed {
        return rows;
    }
    if panel.error.is_some() {
        rows.push(row(item("Task refresh failed", width, true), None));
    } else if tasks.is_empty() {
        rows.push(row(item("No background commands", width, true), None));
    }
    for (index, task) in tasks.iter().enumerate() {
        // 1. 【终端侧栏】【状态一致】与后台任务面板共用状态短词和配色
        let (status, color) = crate::cli::repl_background::status_style(&task.status, &palette);
        let label = if task.label.trim().is_empty() {
            &task.command
        } else {
            &task.label
        };
        rows.push(row(
            item(
                &columns(&safe_text(label), &badge(status, color), width),
                width,
                false,
            ),
            Some(SidebarAction::Background(Some(index))),
        ));
    }
    rows
}
