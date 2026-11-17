//! 后台命令概览使用现有侧栏组件，管理入口在空列表和折叠状态下保持可见。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading_with_tone, item, safe_text};
use crate::cli::repl_background::EmbeddedTaskPanel;

/// 【终端侧栏】【后台概览】空任务统一零值计数与静默标题，管理入口补齐引导符对齐第 4 列文本轴。
/// 参数: panel 为任务快照，collapsed 为折叠状态，width 为净宽；返回: 带操作的侧栏行。
pub(super) fn rows(panel: &EmbeddedTaskPanel, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let tasks = panel.tasks();
    let running = tasks.iter().filter(|task| task.status == "running").count();
    let quiet = tasks.is_empty() && panel.error.is_none();
    let count = if tasks.is_empty() {
        "0".to_string()
    } else {
        format!("{running}/{}", tasks.len())
    };
    let mut rows = vec![
        row(
            heading_with_tone(
                super::sidebar_i18n::background_heading(),
                &count,
                collapsed,
                quiet,
                width,
            ),
            Some(SidebarAction::Toggle(SidebarSection::Background)),
        ),
        row(
            item(
                &columns(
                    &format!(
                        "  {}›\x1b[39m {}{}\x1b[39m",
                        fg(palette.subtle),
                        fg(if quiet { palette.muted } else { palette.text }),
                        super::sidebar_i18n::manage_tasks(),
                    ),
                    &badge("/ps", if quiet { palette.subtle } else { palette.muted }),
                    width,
                ),
                width,
                quiet,
            ),
            Some(SidebarAction::Background(None)),
        ),
    ];
    if collapsed || quiet {
        return rows;
    }
    if panel.error.is_some() {
        rows.push(row(
            item(super::sidebar_i18n::task_refresh_failed(), width, true),
            None,
        ));
    }
    for (index, task) in tasks.iter().enumerate() {
        // 1. 【终端侧栏】【状态一致】与后台任务面板共用状态短词和配色
        let (status, color) = crate::cli::repl_background::status_style(&task.status, &palette);
        let glyph = match task.status.as_str() {
            "running" => "●",
            "failed" => "×",
            _ => "○",
        };
        let label = if task.label.trim().is_empty() {
            &task.command
        } else {
            &task.label
        };
        rows.push(row(
            item(
                &columns(
                    &format!("  {}{glyph}\x1b[39m {}", fg(color), safe_text(label)),
                    &badge(status, color),
                    width,
                ),
                width,
                false,
            ),
            Some(SidebarAction::Background(Some(index))),
        ));
    }
    rows
}
