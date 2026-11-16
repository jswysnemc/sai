//! 待办面板的完成进度与任务条目。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{heading_with_tone, item, safe_text};
use crate::render::transcript::TodoSnapshotItem;

/// 【终端侧栏】【待办面板】标题内嵌微缩进度条，无任务时单行静默展示，有任务时按 2 列缩进呈现。
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
    let quiet = todos.is_empty();
    let summary = if quiet {
        "0/0".to_string()
    } else {
        let slots = todos.len().min(5);
        let filled = done.saturating_mul(slots) / todos.len().max(1);
        format!(
            "{}{}{}{}\x1b[39m {done}/{}",
            fg(palette.added),
            "▪".repeat(filled),
            fg(palette.subtle),
            "▫".repeat(slots.saturating_sub(filled)),
            todos.len()
        )
    };
    let mut rows = vec![row(
        heading_with_tone("Todo", &summary, compact || quiet, quiet, width),
        Some(SidebarAction::Toggle(SidebarSection::Todo)),
    )];
    // 1. 【终端侧栏】【空态收敛】无待办任务时不再追加冗余提示行，把高度让渡给活跃区块
    if quiet {
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
        let status = todo.status.as_str();
        // 2. 【终端侧栏】【状态字形】与工具卡共用状态符，统一缩进两列保持层级对齐
        let (marker, text) = match status {
            "completed" => (palette.added, String::new()),
            "cancelled" => (palette.muted, "\x1b[9m".into()),
            "in_progress" => (palette.accent, fg(palette.accent)),
            _ => (palette.muted, String::new()),
        };
        let glyph = crate::render::todo_style::status_glyph(status);
        rows.push(row(
            item(
                &format!(
                    "  {}{glyph}\x1b[39m {text}{}",
                    fg(marker),
                    safe_text(&todo.text)
                ),
                width,
                matches!(status, "completed" | "cancelled"),
            ),
            None,
        ));
    }

    rows
}
