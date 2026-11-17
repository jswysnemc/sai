//! 可复用的多态选择清单：Agent 工具白名单、Skills 暴露级别等场景共用。

mod render;

use anyhow::Result;
use crossterm::event::{KeyCode, MouseEventKind};
use std::io;

use super::input::{read_event, sanitize_paste_single_line, ConfigEvent};
use super::mouse::ClickTracker;
use super::search::{insert_char_at, insert_str_at_char, remove_char_at, remove_char_before};
use render::draw;

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
pub(super) enum Row {
    Toggle(usize),
    Group(String),
    Entry(usize),
}

impl Row {
    fn selectable(&self) -> bool {
        !matches!(self, Row::Group(_))
    }
}

/// 多选清单过滤态的输入法守卫，保证离开页面时关闭输入法。
#[derive(Default)]
struct FilterImeState {
    active: bool,
}

impl FilterImeState {
    fn enter(&mut self) {
        if !self.active {
            self.active = true;
            super::ime::enable_for_edit();
        }
    }

    fn leave(&mut self) {
        if self.active {
            self.active = false;
            super::ime::disable_for_nav();
        }
    }
}

impl Drop for FilterImeState {
    fn drop(&mut self) {
        self.leave();
    }
}

/// 【多态清单】【交互循环】驱动分组清单导航、状态切换、粘贴过滤及两段式 Esc 清空退出。
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
    let mut filter_cursor = 0usize;
    let mut filter_mode = false;
    let mut ime = FilterImeState::default();
    let mut rows = build_rows(toggles, entries, &filter);
    let mut selected = first_selectable(&rows);
    let mut hovered: Option<usize> = None;
    let mut footer_hover: Option<super::mouse::HitRect> = None;
    let mut clicks = ClickTracker::default();
    let mut footer_clicks = ClickTracker::default();
    loop {
        let hitbox = draw(
            stdout,
            title,
            states,
            toggles,
            entries,
            &rows,
            selected,
            hovered,
            footer_hover,
            &filter,
            filter_cursor,
            filter_mode,
        )?;
        let key = match read_event()? {
            ConfigEvent::Resize => continue,
            ConfigEvent::Paste(text) => {
                if filter_mode {
                    let cleaned = sanitize_paste_single_line(&text);
                    if !cleaned.is_empty() {
                        insert_str_at_char(&mut filter, &mut filter_cursor, &cleaned);
                        rows = build_rows(toggles, entries, &filter);
                        selected = first_selectable(&rows);
                    }
                }
                continue;
            }
            ConfigEvent::Key(key) => key.code,
            ConfigEvent::Mouse(mouse) => {
                // 1. 【多态清单】【悬停追踪】同步追踪候选行（跳过分组标题）与底栏按钮
                if matches!(mouse.kind, MouseEventKind::Moved) {
                    let next = hitbox
                        .list
                        .row_at(mouse.column, mouse.row)
                        .filter(|idx| rows.get(*idx).is_some_and(Row::selectable));
                    let next_footer = hitbox.footer.rect_at(mouse.column, mouse.row);
                    if next != hovered || next_footer != footer_hover {
                        hovered = next;
                        footer_hover = next_footer;
                    }
                    continue;
                }
                // 2. 【多态清单】【滚轮浏览】跳过不可选的分组标题行上下移动
                if hitbox.list.frame.contains(mouse.column, mouse.row) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            selected = previous_selectable(&rows, selected);
                            continue;
                        }
                        MouseEventKind::ScrollDown => {
                            selected = next_selectable(&rows, selected);
                            continue;
                        }
                        _ => {}
                    }
                }
                // 3. 【多态清单】【底栏点击】响应底栏快捷键胶囊点击
                let footer_target = hitbox.footer.hit(mouse.column, mouse.row);
                if let Some(code) = footer_clicks.update(mouse, footer_target) {
                    code
                } else {
                    // 4. 【多态清单】【列表点击】点击状态圆点列立即切换，点击文字先选中再切换
                    let target = hitbox
                        .list
                        .row_at(mouse.column, mouse.row)
                        .filter(|idx| rows.get(*idx).is_some_and(Row::selectable));
                    let Some(index) = clicks.update(mouse, target) else {
                        continue;
                    };
                    if filter_mode {
                        filter_mode = false;
                        ime.leave();
                    }
                    let in_mark_col = mouse.column < hitbox.list.list.x.saturating_add(5);
                    if index == selected || in_mark_col {
                        selected = index;
                        KeyCode::Char(' ')
                    } else {
                        selected = index;
                        continue;
                    }
                }
            }
        };
        if filter_mode {
            let len = filter.chars().count();
            filter_cursor = filter_cursor.min(len);
            let query_changed = match key {
                KeyCode::Esc => {
                    filter_mode = false;
                    ime.leave();
                    filter.clear();
                    filter_cursor = 0;
                    true
                }
                KeyCode::Enter => {
                    filter_mode = false;
                    ime.leave();
                    false
                }
                KeyCode::Left => {
                    filter_cursor = filter_cursor.saturating_sub(1);
                    false
                }
                KeyCode::Right => {
                    filter_cursor = (filter_cursor + 1).min(len);
                    false
                }
                KeyCode::Home => {
                    filter_cursor = 0;
                    false
                }
                KeyCode::End => {
                    filter_cursor = len;
                    false
                }
                KeyCode::Backspace => {
                    if filter_cursor > 0 {
                        remove_char_before(&mut filter, &mut filter_cursor);
                    }
                    if filter.is_empty() {
                        filter_mode = false;
                        ime.leave();
                    }
                    true
                }
                KeyCode::Delete => {
                    if filter_cursor < len {
                        remove_char_at(&mut filter, filter_cursor);
                        true
                    } else {
                        false
                    }
                }
                KeyCode::Up | KeyCode::Down => false,
                KeyCode::Char(ch) => {
                    insert_char_at(&mut filter, &mut filter_cursor, ch);
                    true
                }
                _ => false,
            };
            if matches!(key, KeyCode::Up | KeyCode::Down) {
                rows = build_rows(toggles, entries, &filter);
                selected = match key {
                    KeyCode::Up => previous_selectable(&rows, selected),
                    KeyCode::Down => next_selectable(&rows, selected),
                    _ => selected,
                };
                continue;
            }
            rows = build_rows(toggles, entries, &filter);
            selected = if query_changed {
                first_selectable(&rows)
            } else {
                clamp_selection(&rows, selected)
            };
            continue;
        }
        match key {
            // 5. 【多态清单】【两段式Esc】保留过滤词时首次按 Esc 先清空过滤词恢复完整列表，再按 Esc 才退出
            KeyCode::Esc if !filter.is_empty() => {
                filter.clear();
                filter_cursor = 0;
                rows = build_rows(toggles, entries, &filter);
                selected = clamp_selection(&rows, selected);
            }
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('s') => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') => selected = previous_selectable(&rows, selected),
            KeyCode::Down | KeyCode::Char('j') => selected = next_selectable(&rows, selected),
            KeyCode::Home => selected = first_selectable(&rows),
            KeyCode::End => selected = last_selectable(&rows),
            KeyCode::Char('/') => {
                filter_mode = true;
                filter.clear();
                filter_cursor = 0;
                ime.enter();
                rows = build_rows(toggles, entries, &filter);
                selected = clamp_selection(&rows, selected);
            }
            KeyCode::Char('A') => {
                if states.len() > 1 {
                    for index in visible_entry_indices(&rows) {
                        entries[index].state = 1;
                    }
                }
            }
            KeyCode::Char('0') => {
                for index in visible_entry_indices(&rows) {
                    entries[index].state = 0;
                }
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

/// 取出当前可见（未被过滤掉）的条目下标。
fn visible_entry_indices(rows: &[Row]) -> Vec<usize> {
    rows.iter()
        .filter_map(|row| match row {
            Row::Entry(index) => Some(*index),
            _ => None,
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::render::{format_row_text, state_summary};
    use super::*;
    use crate::config_tui::theme::{ThemePalette, RESET, SELECTION_BAR};

    fn entry(key: &str, group: &str, state: usize) -> SelectEntry {
        SelectEntry {
            key: key.to_string(),
            description: format!("{key} 描述"),
            group_label: group.to_string(),
            state,
        }
    }

    #[test]
    fn rows_group_headers_are_skipped_in_navigation() {
        let entries = vec![
            entry("read_file", "基础", 1),
            entry("write_file", "基础", 0),
            entry("web_search", "网络", 0),
        ];
        let rows = build_rows(&[], &entries, "");

        assert_eq!(rows.len(), 5);
        assert_eq!(first_selectable(&rows), 1);
        assert_eq!(next_selectable(&rows, 2), 4);
        assert_eq!(previous_selectable(&rows, 4), 2);
        assert_eq!(next_selectable(&rows, 4), 4);
    }

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

        let rows = build_rows(&toggles, &entries, "描述");
        let entry_count = rows
            .iter()
            .filter(|row| matches!(row, Row::Entry(_)))
            .count();
        assert_eq!(entry_count, 2);
    }

    #[test]
    fn clamp_selection_lands_on_selectable_rows() {
        let entries = vec![entry("a", "组一", 0), entry("b", "组二", 0)];
        let rows = build_rows(&[], &entries, "");
        assert_eq!(clamp_selection(&rows, 2), 3);
        assert_eq!(clamp_selection(&rows, 9), 3);
        assert_eq!(clamp_selection(&[], 3), 0);
    }

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

    #[test]
    fn summary_counts_the_default_state_too() {
        let states = vec![
            StateStyle {
                mark: "○",
                label: "隐藏",
                color: "",
            },
            StateStyle {
                mark: "●",
                label: "启用",
                color: "",
            },
        ];
        let entries = vec![entry("a", "", 0), entry("b", "", 0), entry("c", "", 1)];

        let summary = state_summary(&states, &entries);
        assert!(summary.contains("○ 隐藏 2"), "{summary}");
        assert!(summary.contains("● 启用 1"), "{summary}");
    }

    #[test]
    fn visible_entry_indices_skips_group_headers_and_toggles() {
        let rows = vec![
            Row::Toggle(0),
            Row::Group("group".to_string()),
            Row::Entry(3),
            Row::Group("other".to_string()),
            Row::Entry(7),
        ];

        assert_eq!(visible_entry_indices(&rows), vec![3, 7]);
    }

    /// 【多态清单】【连续背景测试】验证悬停行与选中行不会因中间 RESET 产生斑马背景断层。
    #[test]
    fn hovered_and_selected_rows_preserve_continuous_background() {
        let palette = ThemePalette::current();
        let states = [StateStyle {
            mark: "●",
            label: "启用",
            color: palette.ok,
        }];
        let entries = vec![entry("read_file", "", 0)];

        let hovered = format_row_text(&states, &[], &entries, &Row::Entry(0), false, true, 40);
        assert!(!crate::config_tui::ui::strip_ansi_plain(&hovered).contains(SELECTION_BAR));
        let hover_body = hovered.strip_suffix(RESET).unwrap();
        assert!(!hover_body.contains(&format!("{RESET} ")));
        assert!(hover_body.contains(&format!("{RESET}{}", palette.hover_bg)));

        let selected = format_row_text(&states, &[], &entries, &Row::Entry(0), true, false, 40);
        assert!(crate::config_tui::ui::strip_ansi_plain(&selected).contains(SELECTION_BAR));
        let sel_body = selected.strip_suffix(RESET).unwrap();
        assert!(!sel_body.contains(&format!("{RESET} ")));
    }
}
