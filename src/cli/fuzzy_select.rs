use super::*;

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
            code, modifiers, ..
        }) = event::read()?
        {
            match code {
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Esc => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Char('q') if query.is_empty() => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Enter => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(matches.get(selected).map(|(_, index)| *index));
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
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
    queue!(
        stdout,
        MoveTo(0, anchor_y),
        Print(truncate_display(&format!("> {query}"), width)),
    )?;
    if matches.is_empty() {
        queue!(
            stdout,
            MoveTo(0, anchor_y + 1),
            Print(t("  no matches", "  没有匹配项"))
        )?;
    } else {
        for (row, (_, item_index)) in matches.iter().skip(scroll).take(visible).enumerate() {
            let item_position = scroll + row;
            let marker = if item_position == selected { ">" } else { " " };
            let line = truncate_display(&format!("{marker} {}", items[*item_index]), width);
            queue!(stdout, MoveTo(0, anchor_y + row as u16 + 1))?;
            if item_position == selected {
                queue!(
                    stdout,
                    SetAttribute(Attribute::Reverse),
                    Print(line),
                    SetAttribute(Attribute::Reset)
                )?;
            } else {
                queue!(stdout, Print(line))?;
            }
        }
    }
    queue!(
        stdout,
        MoveTo(0, anchor_y + menu_lines.saturating_sub(1)),
        Print(truncate_display(
            t(
                "[type] search  [j/k] move  [enter] select  [esc/q] cancel",
                "[输入] 搜索  [j/k] 移动  [enter] 选择  [esc/q] 取消",
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
    for _ in 1..lines {
        println!();
    }
    io::stdout().flush()?;
    Ok(())
}

fn inline_fuzzy_lines(item_count: usize) -> u16 {
    ((item_count.min(10) + 2) as u16).max(3)
}

fn truncate_display(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    }
}

struct InlineRawMode {
    stdout: io::Stdout,
}

impl InlineRawMode {
    fn start() -> Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self {
            stdout: io::stdout(),
        })
    }
}

impl Drop for InlineRawMode {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::inline_fuzzy_scroll_offset;

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
}
