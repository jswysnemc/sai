use super::*;

/// 与 `sai models` 一致的选中高亮（无反色黑底）。
const FOCUS_STYLE: &str = "\x1b[1m\x1b[38;2;190;246;255m";
const DIM_STYLE: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// 内联模糊选择列表，供 `/agent` `/resume` `/tree` `/thinking` 等共用。
///
/// 视觉对齐 models 选择器：`→` + 亮色选中、弱化未选中，不铺反色黑底；
/// 可在 REPL 已启用 raw mode 时嵌套调用。
///
/// 参数:
/// - `items`: 候选项展示文本
///
/// 返回:
/// - 选中项下标；取消时返回空
pub(super) fn inline_fuzzy_select(items: &[String]) -> Result<Option<usize>> {
    let menu_lines = inline_fuzzy_lines(items.len());
    reserve_inline_fuzzy_space(menu_lines)?;
    let mut session = InlineRawMode::start()?;
    let matcher = SkimMatcherV2::default();
    let mut query = String::new();
    let mut selected = 0usize;
    let (_, cursor_y) = cursor::position().unwrap_or((0, menu_lines.saturating_sub(1)));
    let anchor_y = cursor_y.saturating_sub(menu_lines.saturating_sub(1));

    loop {
        let matches = fuzzy_matches(&matcher, items, &query);
        if selected >= matches.len() {
            selected = matches.len().saturating_sub(1);
        }
        draw_inline_fuzzy(
            &mut session.stdout,
            anchor_y,
            menu_lines,
            &query,
            items,
            &matches,
            selected,
        )?;
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = event::read()?
        {
            if !should_process_inline_fuzzy_key(kind) {
                continue;
            }
            match code {
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Esc => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Enter => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(matches.get(selected).map(|(_, index)| *index));
                }
                // 导航只用方向键与 Ctrl 组合：q/j/k 必须留给搜索输入
                KeyCode::Up => selected = selected.saturating_sub(1),
                KeyCode::Down => selected = (selected + 1).min(matches.len().saturating_sub(1)),
                KeyCode::Char('p') if modifiers.contains(KeyModifiers::CONTROL) => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                    selected = (selected + 1).min(matches.len().saturating_sub(1));
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    query.push(ch);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

/// 判断内联选择器是否应处理终端按键事件。
///
/// 参数:
/// - `kind`: 终端上报的按键事件类型
///
/// 返回:
/// - 按下或重复事件返回 true，释放事件返回 false
fn should_process_inline_fuzzy_key(kind: KeyEventKind) -> bool {
    kind != KeyEventKind::Release
}

fn fuzzy_matches(matcher: &SkimMatcherV2, items: &[String], query: &str) -> Vec<(i64, usize)> {
    let mut matches = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if query.trim().is_empty() {
                Some((0, index))
            } else {
                matcher.fuzzy_match(item, query).map(|score| (score, index))
            }
        })
        .collect::<Vec<_>>();
    if !query.trim().is_empty() {
        matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    }
    matches
}

fn draw_inline_fuzzy(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    menu_lines: u16,
    query: &str,
    items: &[String],
    matches: &[(i64, usize)],
    selected: usize,
) -> Result<()> {
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    let width = cols.saturating_sub(2).max(24) as usize;
    let visible = matches.len().min(menu_lines.saturating_sub(2) as usize);
    let scroll = inline_fuzzy_scroll_offset(selected, visible);
    queue!(stdout, Hide)?;
    for row in 0..menu_lines {
        queue!(
            stdout,
            MoveTo(0, anchor_y + row),
            Clear(ClearType::CurrentLine)
        )?;
    }

    // 过滤行：与 models 选择器同款弱化标签 + 亮色输入
    let filter_label = if query.is_empty() {
        t("type to filter", "输入内容过滤")
    } else {
        query
    };
    let header = format!(
        "{DIM_STYLE}{}:{RESET} {FOCUS_STYLE}{filter_label}{RESET}  {DIM_STYLE}({}/{}){RESET}",
        t("Filter", "过滤"),
        matches.len(),
        items.len()
    );
    queue!(
        stdout,
        MoveTo(0, anchor_y),
        Print(truncate_display(&header, width)),
    )?;

    if matches.is_empty() {
        queue!(
            stdout,
            MoveTo(0, anchor_y + 1),
            Print(format!(
                "{DIM_STYLE}{}{RESET}",
                t("  no matches", "  没有匹配项")
            ))
        )?;
    } else {
        for (row, (_, item_index)) in matches.iter().skip(scroll).take(visible).enumerate() {
            let item_position = scroll + row;
            let selected_row = item_position == selected;
            let marker = if selected_row { "→" } else { " " };
            let style = if selected_row { FOCUS_STYLE } else { DIM_STYLE };
            let line = truncate_display(&format!("{marker} {}", items[*item_index]), width);
            queue!(
                stdout,
                MoveTo(0, anchor_y + row as u16 + 1),
                Print(format!("{style}{line}{RESET}"))
            )?;
        }
    }
    queue!(
        stdout,
        MoveTo(0, anchor_y + menu_lines.saturating_sub(1)),
        Print(truncate_display(
            &format!(
                "{DIM_STYLE}{}{RESET}",
                t(
                    "Type to filter · ↑/↓ choose · Enter select · Esc cancel",
                    "输入过滤 · ↑/↓ 选择 · Enter 确认 · Esc 取消",
                )
            ),
            width
        ))
    )?;
    stdout.flush()?;
    Ok(())
}

/// 计算模糊选择列表的可视窗口起始位置。
///
/// 参数:
/// - `selected`: 当前选中项在匹配结果中的索引
/// - `visible`: 可视区域能够展示的项目数量
///
/// 返回:
/// - 匹配结果中第一条可见项目的索引
fn inline_fuzzy_scroll_offset(selected: usize, visible: usize) -> usize {
    if visible == 0 {
        0
    } else {
        selected.saturating_add(1).saturating_sub(visible)
    }
}

fn clear_inline_fuzzy(stdout: &mut io::Stdout, anchor_y: u16, lines: u16) -> Result<()> {
    for row in 0..lines {
        queue!(
            stdout,
            MoveTo(0, anchor_y + row),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stdout, MoveTo(0, anchor_y), Show)?;
    stdout.flush()?;
    Ok(())
}

fn reserve_inline_fuzzy_space(lines: u16) -> Result<()> {
    // raw mode 下用 \r\n 推进，避免 println 依赖 cooked 模式
    let mut stdout = io::stdout();
    for _ in 1..lines {
        queue!(stdout, Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(())
}

fn inline_fuzzy_lines(item_count: usize) -> u16 {
    ((item_count.min(10) + 2) as u16).max(3)
}

fn truncate_display(value: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    if UnicodeWidthStr::width(value) <= max {
        return value.to_string();
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > max.saturating_sub(1) {
            break;
        }
        output.push(ch);
        used += char_width;
    }
    output.push('…');
    output
}

struct InlineRawMode {
    stdout: io::Stdout,
    was_raw: bool,
}

impl InlineRawMode {
    fn start() -> Result<Self> {
        let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
        if !was_raw {
            terminal::enable_raw_mode()?;
        }
        Ok(Self {
            stdout: io::stdout(),
            was_raw,
        })
    }
}

impl Drop for InlineRawMode {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
        // 仅恢复调用前状态：嵌套在 REPL raw mode 时不要关掉
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{inline_fuzzy_scroll_offset, should_process_inline_fuzzy_key};
    use crossterm::event::KeyEventKind;

    /// Windows 上报回车释放事件时，选择器不能将其解释为确认。
    #[test]
    fn fuzzy_selector_ignores_windows_enter_release_event() {
        assert!(!should_process_inline_fuzzy_key(KeyEventKind::Release));
    }

    /// Windows 的一次回车按下与释放序列只能产生一次确认动作。
    #[test]
    fn fuzzy_selector_confirms_windows_enter_sequence_once() {
        assert_eq!(
            [KeyEventKind::Press, KeyEventKind::Release]
                .into_iter()
                .filter(|kind| should_process_inline_fuzzy_key(*kind))
                .count(),
            1
        );
    }

    #[test]
    fn fuzzy_selector_keeps_initial_items_visible() {
        assert_eq!(inline_fuzzy_scroll_offset(0, 10), 0);
        assert_eq!(inline_fuzzy_scroll_offset(9, 10), 0);
    }

    #[test]
    fn fuzzy_selector_scrolls_after_visible_window() {
        assert_eq!(inline_fuzzy_scroll_offset(10, 10), 1);
        assert_eq!(inline_fuzzy_scroll_offset(19, 10), 10);
    }

    #[test]
    fn fuzzy_selector_handles_empty_visible_window() {
        assert_eq!(inline_fuzzy_scroll_offset(4, 0), 0);
    }

    /// 选中行样式不含反色，使用箭头标记。
    #[test]
    fn selected_row_uses_arrow_without_reverse_video() {
        let line = format!("{}→ item{}", super::FOCUS_STYLE, super::RESET);
        assert!(line.contains('→'));
        assert!(!line.contains("\x1b[7m"));
        assert!(line.contains("190;246;255"));
    }
}
