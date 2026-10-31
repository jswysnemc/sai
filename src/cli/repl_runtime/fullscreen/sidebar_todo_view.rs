//! 待办面板的完成进度与任务条目。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{heading, item, safe_text};
use crate::render::transcript::TodoSnapshotItem;

/// 【终端侧栏】【待办面板】以完成计数和状态条目呈现任务，紧凑模式只保留当前项。
/// 参数: todos 为任务，compact 为紧凑模式，width 为宽度，frame 为动画帧；返回: 面板行。
pub(super) fn rows(
    todos: &[TodoSnapshotItem],
    compact: bool,
    width: usize,
    _frame: usize,
) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let done = todos
        .iter()
        .filter(|todo| todo.status == "completed")
        .count();
    let mut rows = vec![row(
        heading("Todo", &format!("{done}/{}", todos.len()), compact, width),
        Some(SidebarAction::Toggle(SidebarSection::Todo)),
    )];
    if todos.is_empty() {
        rows.push(row(item("No active tasks", width, true), None));
        return rows;
    }
    let statuses = todos
        .iter()
        .map(|todo| todo.status.as_str())
        .collect::<Vec<_>>();
    for index in crate::render::todo_style::display_order(&statuses)
        .into_iter()
        .take(if compact { 1 } else { todos.len() })
    {
        let todo = &todos[index];
        let marker = match todo.status.as_str() {
            "completed" => format!("{}[x]", fg(palette.added)),
            "cancelled" => format!("{}[-]", fg(palette.muted)),
            "in_progress" => format!("{}[>]", fg(palette.accent)),
            _ => format!("{}[ ]", fg(palette.muted)),
        };
        rows.push(row(
            item(
                &format!("{marker} {}", safe_text(&todo.text)),
                width,
                todo.status == "completed",
            ),
            None,
        ));
    }

    rows
}
