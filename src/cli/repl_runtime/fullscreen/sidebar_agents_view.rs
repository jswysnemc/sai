//! 固定会话标题与紧凑子任务条目。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::SidebarPalette;
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading, item, safe_text};
use crate::cli::repl_runtime::panel_mouse::PanelAction;
use crate::render::transcript::SubagentOverviewEntry;

/// 【终端侧栏】【会话分组】每个会话只占一行，列表单独滚动且标题固定。
/// 参数: agents 为子任务，viewing 为当前任务，collapsed 为折叠状态，width 为宽度；返回: 分组文档。
pub(super) fn rows(
    agents: &[SubagentOverviewEntry],
    viewing: Option<&str>,
    collapsed: bool,
    width: usize,
) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let mut rows = vec![row(
        heading("Subagents", &agents.len().to_string(), collapsed, width),
        Some(SidebarAction::Toggle(SidebarSection::Agents)),
    )];
    let mut main = row(
        item(
            &columns(
                "Main conversation",
                if viewing.is_none() { "active" } else { "" },
                width,
            ),
            width,
            false,
        ),
        Some(SidebarAction::Panel(PanelAction::Agent(None))),
    );
    main.selected = viewing.is_none();
    rows.push(main);
    if !collapsed {
        for entry in agents {
            let (label, color) = match entry.status {
                "err" => ("error", palette.removed),
                "idle" => ("idle", palette.muted),
                _ if entry.running => ("running", palette.accent),
                _ => ("done", palette.added),
            };
            let status = entry
                .progress
                .map(|(step, max)| format!("{label} {step}/{max}"))
                .unwrap_or_else(|| label.into());
            let mut title = row(
                item(
                    &columns(&safe_text(&entry.label), &badge(&status, color), width),
                    width,
                    false,
                ),
                Some(SidebarAction::Panel(PanelAction::Agent(Some(
                    entry.cell_index,
                )))),
            );
            title.selected = entry.viewing;
            rows.push(title);
        }
    }
    rows
}
