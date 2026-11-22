//! 全屏右侧栏 Super-Agent 群组分组渲染：展示快捷入口与活跃群组/Runner 状态。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_groups::SidebarGroups;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{badge, columns, heading_with_tone, item, safe_text};

/// 【终端侧栏】【群组概览】渲染 Super-Agent 群组入口与活跃群组状态行。
///
/// 参数:
/// - `groups`: 侧栏群组快照
/// - `collapsed`: 是否折叠
/// - `width`: 侧栏净宽
///
/// 返回:
/// - 带点击动作的侧栏行
pub(super) fn rows(groups: &SidebarGroups, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let entries = &groups.entries;
    let running: usize = entries.iter().map(|item| item.running_worker_count).sum();
    let quiet = entries.is_empty();
    let count = if entries.is_empty() {
        "0".to_string()
    } else if running > 0 {
        format!("{running}r/{}", entries.len())
    } else {
        entries.len().to_string()
    };

    let mut rows = vec![
        row(
            heading_with_tone(
                super::sidebar_i18n::groups_heading(),
                &count,
                collapsed,
                quiet,
                width,
            ),
            Some(SidebarAction::Toggle(SidebarSection::Groups)),
        ),
        row(
            item(
                &columns(
                    &format!(
                        "  {}›\x1b[39m {}{}\x1b[39m",
                        fg(palette.subtle),
                        fg(if quiet { palette.muted } else { palette.text }),
                        super::sidebar_i18n::open_groups(),
                    ),
                    &badge("/group", if quiet { palette.subtle } else { palette.muted }),
                    width,
                ),
                width,
                quiet,
            ),
            Some(SidebarAction::Group(None)),
        ),
    ];
    if collapsed || quiet {
        return rows;
    }

    for (index, entry) in entries.iter().take(6).enumerate() {
        let (glyph, color, status_text) = if entry.running_worker_count > 0 {
            (
                "●",
                palette.accent,
                format!("{}r", entry.running_worker_count),
            )
        } else if entry.open_task_count > 0 {
            ("◆", palette.warning, format!("{}t", entry.open_task_count))
        } else {
            ("○", palette.muted, format!("{}m", entry.worker_count + 1))
        };
        rows.push(row(
            item(
                &columns(
                    &format!(
                        "  {}{glyph}\x1b[39m {}",
                        fg(color),
                        safe_text(&entry.group.name)
                    ),
                    &badge(&status_text, color),
                    width,
                ),
                width,
                false,
            ),
            Some(SidebarAction::Group(Some(index))),
        ));
    }
    rows
}
