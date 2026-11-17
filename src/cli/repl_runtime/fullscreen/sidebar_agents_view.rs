//! 固定会话标题与紧凑子任务条目。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading_with_tone, item, safe_text};
use crate::cli::repl_runtime::panel_mouse::PanelAction;
use crate::render::transcript::SubagentOverviewEntry;

/// 【终端侧栏】【会话分组】空载无子任务时自动收敛为单行摘要，有子任务时保留主会话与脉冲状态。
/// 参数: agents 为子任务，viewing 为当前任务，collapsed 为折叠状态，width 为宽度，frame 为动效帧号；返回: 分组文档。
pub(super) fn rows(
    agents: &[SubagentOverviewEntry],
    viewing: Option<&str>,
    collapsed: bool,
    width: usize,
    frame: usize,
) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let quiet = agents.is_empty() && viewing.is_none();
    let mut rows = vec![row(
        heading_with_tone(
            super::sidebar_i18n::subagents_heading(),
            &agents.len().to_string(),
            collapsed || quiet,
            quiet,
            width,
        ),
        Some(SidebarAction::Toggle(SidebarSection::Agents)),
    )];
    // 1. 【终端侧栏】【空态收敛】无子任务且处于主会话时不冗余占行，避免与主画布重复表达
    if quiet {
        return rows;
    }
    // 2. 【终端侧栏】【当前会话】主会话固定在首位，使用 2 列缩进与语义圆点对齐子任务行
    let current = if viewing.is_none() {
        badge(super::sidebar_i18n::agent_active_badge(), palette.accent)
    } else {
        String::new()
    };
    let main_dot = if viewing.is_none() {
        format!("  {}●\x1b[39m ", fg(palette.accent))
    } else {
        format!("  {}○\x1b[39m ", fg(palette.muted))
    };
    let mut main = row(
        item(
            &columns(
                &format!("{main_dot}{}", super::sidebar_i18n::main_conversation()),
                &current,
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
            let (label, glyph, color) = match entry.status {
                "err" => (
                    super::sidebar_i18n::subagent_status_label("err"),
                    "×",
                    palette.removed,
                ),
                "idle" => (
                    super::sidebar_i18n::subagent_status_label("idle"),
                    "○",
                    palette.muted,
                ),
                _ if entry.running => ("", "", palette.accent),
                _ => (
                    super::sidebar_i18n::subagent_status_label("done"),
                    "✓",
                    palette.added,
                ),
            };
            let status_text = entry
                .progress
                .map(|(step, max)| format!("{step}/{max}"))
                .unwrap_or_else(|| label.into());
            let prefix = if entry.running {
                format!(
                    "  {} ",
                    crate::render::activity_indicator::render_activity_indicator(frame)
                )
            } else {
                format!("  {}{glyph}\x1b[39m ", fg(color))
            };
            let right = badge(&status_text, color);
            let mut title = row(
                item(
                    &columns(
                        &format!("{prefix}{}", safe_text(&entry.label)),
                        &right,
                        width,
                    ),
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
