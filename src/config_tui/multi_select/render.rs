//! 【配置界面】【多选清单绘制】绘制多态选择列表、连续整行底色、右侧详情及搜索真实光标。

use super::{HeaderToggle, Row, SelectEntry, StateStyle};
use crate::config_tui::layout::{full_frame, master_detail_widths, scroll_start};
use crate::config_tui::mouse::{HitRect, ListHitbox};
use crate::config_tui::search::{split_at_char, SEARCH_CURSOR_MARK};
use crate::config_tui::theme::{help_line, row_marks, tint_bg, ThemePalette, BOLD, RESET};
use crate::config_tui::ui::{
    begin_synced_frame, display_width, draw_box, draw_scroll_indicator,
    draw_status_bar_with_cursor, draw_wrapped_detail, end_synced_frame, pad, status_bar_hits,
    truncate, MenuHitbox,
};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

/// 【多态清单】【状态统计】统计各状态条目数量，组装副标题（如 `● 启用 12 · ◐ 延迟 5 · 共 58`）。
///
/// 参数:
/// - `states`: 状态样式数组
/// - `entries`: 清单条目切片
///
/// 返回:
/// - 汇总字符串
pub(super) fn state_summary(states: &[StateStyle], entries: &[SelectEntry]) -> String {
    let mut parts = Vec::new();
    for (index, state) in states.iter().enumerate() {
        let count = entries.iter().filter(|entry| entry.state == index).count();
        if count > 0 {
            parts.push(format!("{} {} {count}", state.mark, state.label));
        }
    }
    if parts.is_empty() {
        format!("{} {}", entries.len(), t("items", "项"))
    } else {
        format!(
            "{} · {} {}",
            parts.join(" · "),
            t("total", "共"),
            entries.len()
        )
    }
}

/// 【多态清单】【界面绘制】渲染完整多态清单界面并在过滤输入态定位真实硬件光标。
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    stdout: &mut io::Stdout,
    title: &str,
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    rows: &[Row],
    selected: usize,
    hovered: Option<usize>,
    footer_hover: Option<HitRect>,
    filter: &str,
    filter_cursor: usize,
    filter_mode: bool,
) -> Result<MenuHitbox> {
    let palette = ThemePalette::current();
    let (cols, terminal_rows) = terminal::size()?;
    let frame = full_frame(cols, terminal_rows);
    begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, frame.x, frame.y, frame.width, frame.height, title)?;

    let inner_x = frame.x.saturating_add(2);
    let inner_w = frame.width.saturating_sub(4);
    let mut body_y = frame.y.saturating_add(1);
    let subtitle = if filter.is_empty() {
        state_summary(states, entries)
    } else {
        let visible = rows
            .iter()
            .filter(|row| matches!(row, Row::Entry(_)))
            .count();
        format!(
            "{} /{filter} · {visible} {}",
            t("filter", "过滤"),
            t("matches", "项命中")
        )
    };
    queue!(
        stdout,
        MoveTo(inner_x, body_y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{}{RESET}",
                palette.muted,
                truncate(&subtitle, inner_w as usize)
            ))
            .to_string()
        ))
    )?;
    body_y = body_y.saturating_add(2);

    let body_bottom = frame.y.saturating_add(frame.height.saturating_sub(2));
    let body_h = body_bottom.saturating_sub(body_y).max(1);
    let (left_w, right_w) = master_detail_widths(inner_w);

    let visible_rows = body_h as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        let row_y = body_y.saturating_add(row as u16);
        queue!(stdout, MoveTo(inner_x, row_y))?;
        if index >= rows.len() {
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(" ".repeat(left_w as usize)).to_string()
                ))
            )?;
            continue;
        }
        let line_text = format_row_text(
            states,
            toggles,
            entries,
            &rows[index],
            index == selected,
            hovered == Some(index),
            left_w,
        );
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(&line_text))
        )?;
    }
    draw_scroll_indicator(
        stdout,
        inner_x.saturating_add(left_w),
        body_y,
        body_h,
        rows.len(),
        start,
        visible_rows,
    )?;

    if right_w > 0 {
        let detail_x = inner_x.saturating_add(left_w).saturating_add(2);
        let detail = detail_text(states, toggles, entries, rows.get(selected));
        draw_wrapped_detail(stdout, detail_x, body_y, right_w, body_h, &detail)?;
    }

    let help = if filter_mode {
        let query = if filter.is_empty() {
            format!("{SEARCH_CURSOR_MARK}{}", t("type to filter", "输入以过滤"))
        } else {
            let (before, after) = split_at_char(filter, filter_cursor);
            format!("{before}{SEARCH_CURSOR_MARK}{after}")
        };
        help_line(&[
            ("/", &query),
            ("↑↓", t("move", "移动")),
            ("Enter", t("keep filter", "保留过滤")),
            ("Esc", t("clear", "清除")),
        ])
    } else if !filter.is_empty() {
        let cycle = states
            .iter()
            .map(|state| state.label)
            .collect::<Vec<_>>()
            .join("/");
        help_line(&[
            ("Space", &format!("{} ({cycle})", t("cycle", "切换"))),
            ("A", t("select all", "全选")),
            ("0", t("clear all", "全不选")),
            ("/", t("search", "搜索")),
            ("Esc", t("clear filter", "清除过滤")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    } else {
        let cycle = states
            .iter()
            .map(|state| state.label)
            .collect::<Vec<_>>()
            .join("/");
        help_line(&[
            ("Space", &format!("{} ({cycle})", t("cycle", "切换"))),
            ("A", t("select all", "全选")),
            ("0", t("clear all", "全不选")),
            ("/", t("search", "搜索")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    };
    let bar_cursor = draw_status_bar_with_cursor(stdout, &frame, &help, footer_hover)?;
    let footer = status_bar_hits(&frame, &help);
    if let Some((cx, cy)) = bar_cursor {
        queue!(stdout, Show, MoveTo(cx, cy))?;
    } else {
        queue!(stdout, Hide)?;
    }
    end_synced_frame(stdout)?;
    Ok(MenuHitbox {
        list: ListHitbox {
            frame: HitRect::from(frame),
            list: HitRect {
                x: inner_x,
                y: body_y,
                width: left_w,
                height: visible_rows as u16,
            },
            start,
            total: rows.len(),
            visible_rows,
        },
        footer,
    })
}

/// 【多态清单】【行格式化】构造分组标题、开关行或条目行，确保选中态与悬停态整行底色连续贯穿。
///
/// 参数:
/// - `states`: 状态样式切片
/// - `toggles`: 开关切片
/// - `entries`: 条目切片
/// - `row`: 当前行类型
/// - `is_selected`: 是否为选中项
/// - `is_hovered`: 鼠标是否悬停
/// - `width`: 行总列宽
///
/// 返回:
/// - 整行 ANSI 文本
pub(super) fn format_row_text(
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    row: &Row,
    is_selected: bool,
    is_hovered: bool,
    width: u16,
) -> String {
    let palette = ThemePalette::current();
    let width = width as usize;
    match row {
        Row::Group(label) => {
            let name = truncate(label, width.saturating_sub(6));
            let used = 4 + display_width(&name) + 1;
            let tail = width.saturating_sub(used);
            format!(
                "{}── {RESET}{}{BOLD}{name}{RESET} {}{}{RESET}",
                palette.dim,
                palette.brand,
                palette.dim,
                "─".repeat(tail)
            )
        }
        Row::Toggle(index) => {
            let toggle = &toggles[*index];
            let (bar, style) = row_marks(is_selected, is_hovered);
            let (mark, mark_color) = if toggle.value {
                ("●", palette.ok)
            } else {
                ("○", palette.dim)
            };
            let label = truncate(&toggle.label, width.saturating_sub(6));
            if is_selected {
                let line = format!("{mark} {label}");
                let raw = format!(
                    "{bar}{style} {}{RESET}",
                    pad(&line, width.saturating_sub(2))
                );
                tint_bg(&raw, palette.select_bg)
            } else if is_hovered {
                // 1. 【多态清单】【连续悬停】补齐圆点与文字之间的 RESET 底色断层
                let raw = format!(
                    "{bar} {mark_color}{mark}{RESET} {style}{}{RESET}",
                    pad(&label, width.saturating_sub(4))
                );
                tint_bg(&raw, palette.hover_bg)
            } else {
                format!(
                    "{bar} {mark_color}{mark}{RESET} {}{}{RESET}",
                    palette.muted,
                    pad(&label, width.saturating_sub(4))
                )
            }
        }
        Row::Entry(index) => {
            let entry = &entries[*index];
            let (bar, style) = row_marks(is_selected, is_hovered);
            let state = states.get(entry.state);
            let mark = state.map(|state| state.mark).unwrap_or("?");
            let mark_color = state.map(|state| state.color).unwrap_or(palette.muted);
            let label = truncate(&entry.key, width.saturating_sub(6));
            if is_selected {
                let line = format!("{mark} {label}");
                let raw = format!(
                    "{bar}{style} {}{RESET}",
                    pad(&line, width.saturating_sub(2))
                );
                tint_bg(&raw, palette.select_bg)
            } else if is_hovered {
                let raw = format!(
                    "{bar} {mark_color}{mark}{RESET} {style}{}{RESET}",
                    pad(&label, width.saturating_sub(4))
                );
                tint_bg(&raw, palette.hover_bg)
            } else {
                let name_style = if entry.state == 0 { palette.dim } else { RESET };
                format!(
                    "{bar} {mark_color}{mark}{RESET} {name_style}{}{RESET}",
                    pad(&label, width.saturating_sub(4))
                )
            }
        }
    }
}

/// 组装右侧详情文本。
fn detail_text(
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    row: Option<&Row>,
) -> String {
    match row {
        Some(Row::Toggle(index)) => {
            let toggle = &toggles[*index];
            let value = if toggle.value {
                t("on", "开启")
            } else {
                t("off", "关闭")
            };
            format!(
                "{}\n\n{}\n\n{}: {value}",
                toggle.label,
                toggle.description,
                t("Current", "当前")
            )
        }
        Some(Row::Entry(index)) => {
            let entry = &entries[*index];
            let state = states
                .get(entry.state)
                .map(|state| format!("{} {}", state.mark, state.label))
                .unwrap_or_default();
            let mut parts = vec![entry.key.clone()];
            if !entry.group_label.is_empty() {
                parts.push(format!("{}: {}", t("Group", "分组"), entry.group_label));
            }
            parts.push(format!("{}: {state}", t("State", "状态")));
            if !entry.description.trim().is_empty() {
                parts.push(entry.description.clone());
            }
            parts.join("\n\n")
        }
        _ => String::new(),
    }
}
