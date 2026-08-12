//! 可复用的多态选择清单：Agent 工具白名单、Skills 暴露级别等场景共用。
//!
//! 列表项在若干状态间循环（如 隐藏 → 启用 → 延迟），支持分组标题行、
//! 顶部布尔开关行和 `/` 过滤；右侧详情栏展示当前项说明。

use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};

use super::input::read_key;
use super::layout::{full_frame, master_detail_widths, scroll_start};
use super::theme::{help_line, selection_marks, ACCENT, BOLD, BRAND, DIM, MUTED, OK, RESET};
use super::ui::{
    draw_box, draw_scroll_indicator, draw_status_bar, draw_wrapped_detail, pad, truncate,
};

/// 清单条目的一个可循环状态。
pub(super) struct StateStyle {
    /// 列表行状态标记（如 ● ◐ ○）
    pub mark: &'static str,
    /// 状态名，用于帮助条与详情栏
    pub label: &'static str,
    /// 标记颜色
    pub color: &'static str,
}

/// 多态选择清单条目。
pub(super) struct SelectEntry {
    /// 写回配置的标识（同时作为列表显示名）
    pub key: String,
    /// 详情栏描述
    pub description: String,
    /// 分组展示名；相邻同组条目共享一个分组标题行
    pub group_label: String,
    /// 当前状态下标（指向 `states` 数组）
    pub state: usize,
}

/// 清单顶部的布尔开关行（用于通配、模式切换等特殊语义）。
pub(super) struct HeaderToggle {
    pub label: String,
    pub description: String,
    pub value: bool,
}

/// 清单可见行：开关、分组标题或条目。
enum Row {
    Toggle(usize),
    Group(String),
    Entry(usize),
}

impl Row {
    fn selectable(&self) -> bool {
        !matches!(self, Row::Group(_))
    }
}

/// 运行多态选择清单交互循环。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `states`: 状态循环定义，Space 按数组顺序循环
/// - `toggles`: 顶部布尔开关行；可为空
/// - `entries`: 清单条目，需按分组排好顺序
///
/// 返回:
/// - `true` 表示保存（调用方读取 toggles/entries 写回配置）；`false` 表示取消
pub(super) fn run_multi_select(
    stdout: &mut io::Stdout,
    title: &str,
    states: &[StateStyle],
    toggles: &mut [HeaderToggle],
    entries: &mut [SelectEntry],
) -> Result<bool> {
    let mut filter = String::new();
    let mut filter_mode = false;
    let mut rows = build_rows(toggles, entries, &filter);
    let mut selected = first_selectable(&rows);
    loop {
        draw(
            stdout,
            title,
            states,
            toggles,
            entries,
            &rows,
            selected,
            &filter,
            filter_mode,
        )?;
        let key = read_key()?;
        if filter_mode {
            match key {
                KeyCode::Esc => {
                    filter_mode = false;
                    filter.clear();
                }
                KeyCode::Enter => filter_mode = false,
                KeyCode::Backspace => {
                    filter.pop();
                }
                KeyCode::Char(ch) => filter.push(ch),
                _ => {}
            }
            rows = build_rows(toggles, entries, &filter);
            selected = clamp_selection(&rows, selected);
            continue;
        }
        match key {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('s') => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') => selected = previous_selectable(&rows, selected),
            KeyCode::Down | KeyCode::Char('j') => selected = next_selectable(&rows, selected),
            KeyCode::Home => selected = first_selectable(&rows),
            KeyCode::End => selected = last_selectable(&rows),
            KeyCode::Char('/') => {
                filter_mode = true;
                filter.clear();
                rows = build_rows(toggles, entries, &filter);
                selected = clamp_selection(&rows, selected);
            }
            KeyCode::Char(' ') | KeyCode::Enter => match rows.get(selected) {
                Some(Row::Toggle(index)) => toggles[*index].value = !toggles[*index].value,
                Some(Row::Entry(index)) => {
                    entries[*index].state = (entries[*index].state + 1) % states.len().max(1);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// 依据过滤词构建可见行序列，相邻同组条目前插入分组标题。
fn build_rows(toggles: &[HeaderToggle], entries: &[SelectEntry], filter: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for index in 0..toggles.len() {
        rows.push(Row::Toggle(index));
    }
    let filter = filter.to_lowercase();
    let mut last_group: Option<&str> = None;
    for (index, entry) in entries.iter().enumerate() {
        if !filter.is_empty()
            && !entry.key.to_lowercase().contains(&filter)
            && !entry.description.to_lowercase().contains(&filter)
        {
            continue;
        }
        if !entry.group_label.is_empty() && last_group != Some(entry.group_label.as_str()) {
            rows.push(Row::Group(entry.group_label.clone()));
            last_group = Some(entry.group_label.as_str());
        }
        rows.push(Row::Entry(index));
    }
    rows
}

fn first_selectable(rows: &[Row]) -> usize {
    rows.iter().position(Row::selectable).unwrap_or(0)
}

fn last_selectable(rows: &[Row]) -> usize {
    rows.iter().rposition(Row::selectable).unwrap_or(0)
}

fn next_selectable(rows: &[Row], selected: usize) -> usize {
    let mut next = selected;
    loop {
        next += 1;
        if next >= rows.len() {
            return selected;
        }
        if rows[next].selectable() {
            return next;
        }
    }
}

fn previous_selectable(rows: &[Row], selected: usize) -> usize {
    let mut previous = selected;
    while previous > 0 {
        previous -= 1;
        if rows[previous].selectable() {
            return previous;
        }
    }
    selected
}

/// 过滤后行数变化时，把选中项夹回最近的可选行。
fn clamp_selection(rows: &[Row], selected: usize) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let selected = selected.min(rows.len() - 1);
    if rows[selected].selectable() {
        return selected;
    }
    let next = next_selectable(rows, selected);
    if next != selected && rows[next].selectable() {
        return next;
    }
    previous_selectable(rows, selected)
}

/// 统计各状态条目数量，组装副标题（如 `● 启用 12 · ◐ 延迟 5 · 共 58`）。
fn state_summary(states: &[StateStyle], entries: &[SelectEntry]) -> String {
    let mut parts = Vec::new();
    for (index, state) in states.iter().enumerate() {
        // 默认态（第 0 态）不计数，避免副标题充满无信息量的「关闭 41」
        if index == 0 {
            continue;
        }
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

#[allow(clippy::too_many_arguments)]
fn draw(
    stdout: &mut io::Stdout,
    title: &str,
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    rows: &[Row],
    selected: usize,
    filter: &str,
    filter_mode: bool,
) -> Result<()> {
    let (cols, terminal_rows) = terminal::size()?;
    let frame = full_frame(cols, terminal_rows);
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, frame.x, frame.y, frame.width, frame.height, title)?;

    let inner_x = frame.x.saturating_add(2);
    let inner_w = frame.width.saturating_sub(4);
    let mut body_y = frame.y.saturating_add(1);
    // 副标题：状态统计（过滤时展示命中数）
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
        Print(format!(
            "{MUTED}{}{RESET}",
            truncate(&subtitle, inner_w as usize)
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
            queue!(stdout, Print(" ".repeat(left_w as usize)))?;
            continue;
        }
        draw_row(
            stdout,
            states,
            toggles,
            entries,
            &rows[index],
            index == selected,
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
            ("/", t("search", "搜索")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    };
    draw_status_bar(stdout, &frame, &help)?;
    stdout.flush()?;
    Ok(())
}

/// 绘制单行：开关行、分组标题或条目行。
fn draw_row(
    stdout: &mut io::Stdout,
    states: &[StateStyle],
    toggles: &[HeaderToggle],
    entries: &[SelectEntry],
    row: &Row,
    is_selected: bool,
    width: u16,
) -> Result<()> {
    let width = width as usize;
    match row {
        Row::Group(label) => {
            // 分组标题：品牌色短横 + 名称 + 弱化延伸线（与表单分组同款）
            let name = truncate(label, width.saturating_sub(6));
            let used = 4 + super::ui::display_width(&name) + 1;
            let tail = width.saturating_sub(used);
            queue!(
                stdout,
                Print(format!(
                    "{DIM}── {RESET}{BRAND}{BOLD}{name}{RESET} {DIM}{}{RESET}",
                    "─".repeat(tail)
                ))
            )?;
        }
        Row::Toggle(index) => {
            let toggle = &toggles[*index];
            let (bar, style) = selection_marks(is_selected);
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
                    Print(format!(
                        "{bar}{style} {}{RESET}",
                        pad(&line, width.saturating_sub(2))
                    ))
                )?;
            } else {
                queue!(
                    stdout,
                    Print(format!(
                        "{bar} {mark_color}{mark}{RESET} {MUTED}{}{RESET}",
                        pad(&label, width.saturating_sub(4))
                    ))
                )?;
            }
        }
        Row::Entry(index) => {
            let entry = &entries[*index];
            let (bar, style) = selection_marks(is_selected);
            let state = states.get(entry.state);
            let mark = state.map(|state| state.mark).unwrap_or("?");
            let mark_color = state.map(|state| state.color).unwrap_or(MUTED);
            let label = truncate(&entry.key, width.saturating_sub(6));
            if is_selected {
                let line = format!("{mark} {label}");
                queue!(
                    stdout,
                    Print(format!(
                        "{bar}{style} {}{RESET}",
                        pad(&line, width.saturating_sub(2))
                    ))
                )?;
            } else {
                // 未选中：状态标记按语义分色，未启用条目名弱化
                let name_style = if entry.state == 0 { DIM } else { RESET };
                queue!(
                    stdout,
                    Print(format!(
                        "{bar} {mark_color}{mark}{RESET} {name_style}{}{RESET}",
                        pad(&label, width.saturating_sub(4))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, group: &str, state: usize) -> SelectEntry {
        SelectEntry {
            key: key.to_string(),
            description: format!("{key} 描述"),
            group_label: group.to_string(),
            state,
        }
    }

    /// 相邻同组条目只插入一个分组标题，导航自动跳过标题行。
    #[test]
    fn rows_group_headers_are_skipped_in_navigation() {
        let entries = vec![
            entry("read_file", "基础", 1),
            entry("write_file", "基础", 0),
            entry("web_search", "网络", 0),
        ];
        let rows = build_rows(&[], &entries, "");

        // 组标题 + 条目：基础, read, write, 网络, web
        assert_eq!(rows.len(), 5);
        assert_eq!(first_selectable(&rows), 1);
        assert_eq!(next_selectable(&rows, 2), 4);
        assert_eq!(previous_selectable(&rows, 4), 2);
        // 末行再向下停留原地
        assert_eq!(next_selectable(&rows, 4), 4);
    }

    /// 过滤同时匹配条目名与描述，开关行恒可见。
    #[test]
    fn filter_matches_key_and_description() {
        let toggles = vec![HeaderToggle {
            label: "白名单".to_string(),
            description: String::new(),
            value: false,
        }];
        let entries = vec![
            entry("web_search", "网络", 0),
            entry("read_file", "基础", 0),
        ];

        let rows = build_rows(&toggles, &entries, "web");
        let entry_count = rows
            .iter()
            .filter(|row| matches!(row, Row::Entry(_)))
            .count();
        assert_eq!(entry_count, 1);
        assert!(matches!(rows[0], Row::Toggle(0)));

        // 匹配描述
        let rows = build_rows(&toggles, &entries, "描述");
        let entry_count = rows
            .iter()
            .filter(|row| matches!(row, Row::Entry(_)))
            .count();
        assert_eq!(entry_count, 2);
    }

    /// 过滤后选中项夹回最近可选行，不落在分组标题上。
    #[test]
    fn clamp_selection_lands_on_selectable_rows() {
        let entries = vec![entry("a", "组一", 0), entry("b", "组二", 0)];
        let rows = build_rows(&[], &entries, "");
        // rows: 组一, a, 组二, b
        assert_eq!(clamp_selection(&rows, 2), 3);
        assert_eq!(clamp_selection(&rows, 9), 3);
        assert_eq!(clamp_selection(&[], 3), 0);
    }

    /// 副标题统计非默认态数量。
    #[test]
    fn summary_counts_non_default_states() {
        let states = [
            StateStyle {
                mark: "○",
                label: "关闭",
                color: "",
            },
            StateStyle {
                mark: "●",
                label: "启用",
                color: "",
            },
        ];
        let entries = vec![entry("a", "", 1), entry("b", "", 0), entry("c", "", 1)];

        let summary = state_summary(&states, &entries);
        assert!(summary.contains("● 启用 2"));
        assert!(summary.contains('3'));
    }
}
