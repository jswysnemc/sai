//! 【配置界面】【多选清单绘制】绘制多态选择列表、状态图标、右侧详情与底栏操作热区。

use super::{HeaderToggle, Row, SelectEntry, StateStyle};
use crate::config_tui::layout::{full_frame, master_detail_widths, scroll_start};
use crate::config_tui::mouse::{HitRect, ListHitbox};
use crate::config_tui::theme::{help_line, row_marks, ACCENT, BOLD, BRAND, DIM, MUTED, OK, RESET};
use crate::config_tui::ui::{
    draw_box, draw_scroll_indicator, draw_status_bar_interactive, draw_wrapped_detail, pad,
    status_bar_hits, truncate, MenuHitbox,
};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

/// 统计各状态条目数量，组装副标题（如 `● 启用 12 · ◐ 延迟 5 · 共 58`）。
pub(super) fn state_summary(states: &[StateStyle], entries: &[SelectEntry]) -> String {
    let mut parts = Vec::new();
    for (index, state) in states.iter().enumerate() {
        // 1. 每个状态都计数，包括默认态：第 0 态是隐藏，漏掉它无法直接看清隐藏总数
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

/// 【配置界面】【多选绘制】渲染完整多态清单界面并返回列表及底栏命中区。
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
    filter_mode: bool,
) -> Result<MenuHitbox> {
    let (cols, terminal_rows) = terminal::size()?;
    let frame = full_frame(cols, terminal_rows);
    crate::config_tui::ui::begin_synced_frame(stdout)?;
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
            &(format!("{MUTED}{}{RESET}", truncate(&subtitle, inner_w as usize))).to_string()
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
        draw_row(
            stdout,
            states,
            toggles,
            entries,
            &rows[index],
            index == selected,
            hovered == Some(index),
            left_w,
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
        format!(
            "{ACCENT}{}{RESET}{MUTED}: {filter}{RESET}{ACCENT}_{RESET}  {}",
            t("Search", "搜索"),
            help_line(&[("Enter", t("confirm", "确认")), ("Esc", t("clear", "清除")),])
        )
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
    draw_status_bar_interactive(stdout, &frame, &help, footer_hover)?;
    let footer = status_bar_hits(&frame, &help);
    crate::config_tui::ui::end_synced_frame(stdout)?;
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

/// 绘制单行：开关行、分组标题或条目行。
#[allow(clippy::too_many_arguments)]
fn draw_row(
    stdout: &mut io::Stdout,
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    row: &Row,
    is_selected: bool,
    is_hovered: bool,
    width: u16,
) -> Result<()> {
    let width = width as usize;
    match row {
        Row::Group(label) => {
            // 2. 分组标题：品牌色短横 + 名称 + 弱化延伸线
            let name = truncate(label, width.saturating_sub(6));
            let used = 4 + crate::config_tui::ui::display_width(&name) + 1;
            let tail = width.saturating_sub(used);
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(format!(
                        "{DIM}── {RESET}{BRAND}{BOLD}{name}{RESET} {DIM}{}{RESET}",
                        "─".repeat(tail)
                    ))
                    .to_string()
                ))
            )?;
        }
        Row::Toggle(index) => {
            let toggle = &toggles[*index];
            let (bar, style) = row_marks(is_selected, is_hovered);
            let (mark, mark_color) = if toggle.value {
                ("●", OK)
            } else {
                ("○", DIM)
            };
            let label = truncate(&toggle.label, width.saturating_sub(6));
            if is_selected {
                let line = format!("{mark} {label}");
                queue!(
                    stdout,
                    Print(crate::render::readable_terminal_text(
                        &(format!(
                            "{bar}{style} {}{RESET}",
                            pad(&line, width.saturating_sub(2))
                        ))
                        .to_string()
                    ))
                )?;
            } else {
                let label_style = if is_hovered { style } else { MUTED };
                queue!(
                    stdout,
                    Print(crate::render::readable_terminal_text(
                        &(format!(
                            "{bar} {mark_color}{mark}{RESET} {label_style}{}{RESET}",
                            pad(&label, width.saturating_sub(4))
                        ))
                        .to_string()
                    ))
                )?;
            }
        }
        Row::Entry(index) => {
            let entry = &entries[*index];
            let (bar, style) = row_marks(is_selected, is_hovered);
            let state = states.get(entry.state);
            let mark = state.map(|state| state.mark).unwrap_or("?");
            let mark_color = state.map(|state| state.color).unwrap_or(MUTED);
            let label = truncate(&entry.key, width.saturating_sub(6));
            if is_selected {
                let line = format!("{mark} {label}");
                queue!(
                    stdout,
                    Print(crate::render::readable_terminal_text(
                        &(format!(
                            "{bar}{style} {}{RESET}",
                            pad(&line, width.saturating_sub(2))
                        ))
                        .to_string()
                    ))
                )?;
            } else {
                // 3. 未选中条目：悬停时使用提亮样式，未启用条目弱化显示
                let name_style = if is_hovered {
                    style
                } else if entry.state == 0 {
                    DIM
                } else {
                    RESET
                };
                queue!(
                    stdout,
                    Print(crate::render::readable_terminal_text(
                        &(format!(
                            "{bar} {mark_color}{mark}{RESET} {name_style}{}{RESET}",
                            pad(&label, width.saturating_sub(4))
                        ))
                        .to_string()
                    ))
                )?;
            }
        }
    }
    Ok(())
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
