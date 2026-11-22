use super::mouse_menu::{ButtonRow, ClickTracker};
use super::*;
use crate::render::panel_palette::{fg, PanelPalette};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, MouseEventKind};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const RESET: &str = "\x1b[0m";

/// 【模糊选择】【内联入口】内联模糊选择列表，供 `/agent`、`/tree`、`/thinking` 等命令共用。
///
/// 参数:
/// - `items`: 候选项展示文本列表
///
/// 返回:
/// - 选中项在原列表中的下标；取消时返回空
pub(super) fn inline_fuzzy_select(items: &[String]) -> Result<Option<usize>> {
    let mut menu_lines = inline_fuzzy_lines(items.len()).min(terminal::size()?.1);
    reserve_inline_fuzzy_space(menu_lines)?;
    let mut session = InlineRawMode::start()?;
    let matcher = SkimMatcherV2::default();
    let mut query = String::new();
    let mut selected = 0usize;
    let (_, cursor_y) = cursor::position().unwrap_or((0, menu_lines.saturating_sub(1)));
    let mut anchor_y = cursor_y.saturating_sub(menu_lines.saturating_sub(1));
    let mut click = ClickTracker::default();

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
        let input = match crate::cli::terminal_events::read()? {
            Event::Resize(columns, rows) => {
                click.clear();
                clear_inline_fuzzy(
                    &mut session.stdout,
                    anchor_y.min(rows.saturating_sub(1)),
                    menu_lines.min(rows.saturating_sub(anchor_y)),
                )?;
                menu_lines = inline_fuzzy_lines(items.len()).min(rows);
                anchor_y = anchor_y.min(rows.saturating_sub(menu_lines));
                let _ = columns;
                continue;
            }
            Event::Paste(text) => {
                // 1. 【模糊选择】【粘贴净化】过滤回车换行与控制序列，防止误触发提交或破坏终端状态
                click.clear();
                let cleaned = sanitize_paste_query(&text);
                if !cleaned.is_empty() {
                    query.push_str(&cleaned);
                    selected = 0;
                }
                continue;
            }
            Event::Mouse(mouse) => {
                let width = usize::from(terminal::size()?.0.saturating_sub(2));
                let row = mouse.row.checked_sub(anchor_y).map(usize::from);
                let visible = matches.len().min(menu_lines.saturating_sub(2) as usize);
                let scroll = inline_fuzzy_scroll_offset(selected, visible);
                let target = row.and_then(|row| {
                    if row == usize::from(menu_lines.saturating_sub(1)) {
                        return fuzzy_buttons(width).hit(mouse.column);
                    }
                    (row > 0 && row <= visible && usize::from(mouse.column) < width)
                        .then_some(FuzzyTarget::Item(scroll + row.saturating_sub(1)))
                });
                if let Some(target) = click.update(mouse, target) {
                    match target {
                        FuzzyTarget::Item(index) => {
                            // 2. 【模糊选择】【二次点击确认】首次点击移动焦点，再次点击已选中项立即确认
                            if selected == index {
                                clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                                return Ok(matches.get(selected).map(|(_, item_idx)| *item_idx));
                            }
                            selected = index;
                            continue;
                        }
                        FuzzyTarget::Key(code) => {
                            Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
                        }
                    }
                } else {
                    if row.is_some_and(|row| row < usize::from(menu_lines)) {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => selected = selected.saturating_sub(1),
                            MouseEventKind::ScrollDown => {
                                selected = (selected + 1).min(matches.len().saturating_sub(1))
                            }
                            _ => {}
                        }
                    }
                    continue;
                }
            }
            input => {
                click.clear();
                input
            }
        };
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = input
        {
            if !should_process_inline_fuzzy_key(kind) {
                continue;
            }
            match code {
                KeyCode::Char('c' | 'C') if modifiers.contains(KeyModifiers::CONTROL) => {
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
                KeyCode::Char('p' | 'P') if modifiers.contains(KeyModifiers::CONTROL) => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Char('n' | 'N') if modifiers.contains(KeyModifiers::CONTROL) => {
                    selected = (selected + 1).min(matches.len().saturating_sub(1));
                }
                KeyCode::Char('u' | 'U') if modifiers.contains(KeyModifiers::CONTROL) => {
                    query.clear();
                    selected = 0;
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(ch)
                    if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    query.push(ch);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

/// 【模糊选择】【粘贴清理】剥离终端控制序列并过滤换行与控制字符，制表符转为空格。
///
/// 参数:
/// - `text`: 原始粘贴文本
///
/// 返回:
/// - 安全的单行过滤词片段
fn sanitize_paste_query(text: &str) -> String {
    strip_terminal_control_sequences(text)
        .chars()
        .filter_map(|ch| {
            if ch == '\t' {
                Some(' ')
            } else if ch.is_control() {
                None
            } else {
                Some(ch)
            }
        })
        .collect()
}

/// 【模糊选择】【按键过滤】判断内联选择器是否应处理终端按键事件。
///
/// 参数:
/// - `kind`: 终端上报的按键事件类型
///
/// 返回:
/// - 按下或重复事件返回 true，释放事件返回 false
fn should_process_inline_fuzzy_key(kind: KeyEventKind) -> bool {
    kind != KeyEventKind::Release
}

/// 【模糊选择】【匹配排序】按查询词过滤并按匹配得分排序候选项。
///
/// 参数:
/// - `matcher`: 模糊匹配器
/// - `items`: 原始候选文本
/// - `query`: 当前过滤词
///
/// 返回:
/// - `(得分, 原始索引)` 列表
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

/// 【模糊选择】【选中行样式】根据给定调色板渲染单条候选行。
///
/// 参数:
/// - `palette`: 终端自适应调色板
/// - `item`: 候选文本
/// - `selected_row`: 是否为当前选中行
/// - `width`: 最大可见列宽
///
/// 返回:
/// - 带自适应前景样式的单行文本
fn format_candidate_row(
    palette: PanelPalette,
    item: &str,
    selected_row: bool,
    width: usize,
) -> String {
    let marker = if selected_row { "→" } else { " " };
    let style = if selected_row {
        format!("\x1b[1m{}", fg(palette.accent))
    } else {
        fg(palette.muted)
    };
    let line = truncate_display(&format!("{marker} {item}"), width);
    format!("{style}{line}{RESET}")
}

/// 【模糊选择】【界面绘制】按明暗自适应配色重绘内联列表并将光标定位到过滤输入处。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 面板锚点行
/// - `menu_lines`: 面板总行数
/// - `query`: 当前过滤词
/// - `items`: 原始候选文本
/// - `matches`: 匹配结果
/// - `selected`: 当前选中下标
///
/// 返回:
/// - 绘制结果
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
    let width = cols.saturating_sub(2).max(1) as usize;
    let visible = matches.len().min(menu_lines.saturating_sub(2) as usize);
    let scroll = inline_fuzzy_scroll_offset(selected, visible);
    let palette = PanelPalette::current();
    let muted = fg(palette.muted);
    let input_fg = format!("\x1b[1m{}", fg(palette.text));

    queue!(stdout, Hide)?;
    for row in 0..menu_lines {
        queue!(
            stdout,
            MoveTo(0, anchor_y + row),
            Clear(ClearType::CurrentLine)
        )?;
    }

    // 1. 【模糊选择】【过滤行与光标】保留最新输入尾部，计算精确光标列防止输入法候选窗漂移
    let filter_title = format!("{}: ", t("Filter", "过滤"));
    let prefix_width = UnicodeWidthStr::width(filter_title.as_str());
    let count_suffix = format!("  ({}/{})", matches.len(), items.len());
    let query_budget = width
        .saturating_sub(prefix_width + UnicodeWidthStr::width(count_suffix.as_str()))
        .max(4);
    let (rendered_query, cursor_offset) = if query.is_empty() {
        let placeholder = crate::render::clip_to_width(
            t("type to filter", "输入内容过滤"),
            query_budget,
            "…",
        );
        (format!("{muted}{placeholder}{RESET}"), 0usize)
    } else {
        let tail = keep_query_tail(query, query_budget);
        let tail_w = UnicodeWidthStr::width(tail.as_str());
        (format!("{input_fg}{tail}{RESET}"), tail_w)
    };
    let header = format!("{muted}{filter_title}{RESET}{rendered_query}{muted}{count_suffix}{RESET}");
    queue!(
        stdout,
        MoveTo(0, anchor_y),
        Print(truncate_display(&header, width)),
    )?;

    // 2. 【模糊选择】【候选列表】使用明暗自适应强调色绘制选中箭头行
    if matches.is_empty() {
        queue!(
            stdout,
            MoveTo(0, anchor_y + 1),
            Print(format!(
                "{muted}{}{RESET}",
                t("  no matches", "  没有匹配项")
            ))
        )?;
    } else {
        for (row, (_, item_index)) in matches.iter().skip(scroll).take(visible).enumerate() {
            let selected_row = scroll + row == selected;
            queue!(
                stdout,
                MoveTo(0, anchor_y + row as u16 + 1),
                Print(format_candidate_row(
                    palette,
                    &items[*item_index],
                    selected_row,
                    width
                ))
            )?;
        }
    }
    queue!(
        stdout,
        MoveTo(0, anchor_y + menu_lines.saturating_sub(1)),
        Print(truncate_display(
            &format!("{muted}{}{RESET}", fuzzy_buttons(width).text),
            width
        ))
    )?;
    let cursor_x = (prefix_width + cursor_offset).min(usize::from(cols.saturating_sub(1))) as u16;
    queue!(stdout, MoveTo(cursor_x, anchor_y), Show)?;
    stdout.flush()?;
    Ok(())
}

/// 【模糊选择】【尾部保留】当搜索词超出列宽预算时保留最新输入的末尾片段。
///
/// 参数:
/// - `query`: 完整搜索词
/// - `budget`: 可用显示列宽
///
/// 返回:
/// - 不超过预算的搜索词文本
fn keep_query_tail(query: &str, budget: usize) -> String {
    if UnicodeWidthStr::width(query) <= budget {
        return query.to_string();
    }
    if budget <= 1 {
        return "…".repeat(budget);
    }
    let mut kept = Vec::new();
    let mut used = 1usize;
    for ch in query.chars().rev() {
        let ch_w = ch.width().unwrap_or(0);
        if used + ch_w > budget {
            break;
        }
        kept.push(ch);
        used += ch_w;
    }
    format!("…{}", kept.into_iter().rev().collect::<String>())
}

/// 【模糊选择】【滚动窗口】计算模糊选择列表的可视窗口起始位置。
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

/// 【模糊选择】【清理区域】擦除内联列表占用的全部行并恢复光标。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 面板首行
/// - `lines`: 占用行数
///
/// 返回:
/// - 擦除结果
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

/// 【模糊选择】【预留空间】在当前光标下推进所需行数。
///
/// 参数:
/// - `lines`: 面板行数
///
/// 返回:
/// - 推进结果
fn reserve_inline_fuzzy_space(lines: u16) -> Result<()> {
    // raw mode 下用 \r\n 推进，避免 println 依赖 cooked 模式
    let mut stdout = io::stdout();
    for _ in 1..lines {
        queue!(stdout, Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(())
}

/// 【模糊选择】【面板高度】计算给定候选数对应的内联面板行数。
///
/// 参数:
/// - `item_count`: 候选数量
///
/// 返回:
/// - 面板占用行数
fn inline_fuzzy_lines(item_count: usize) -> u16 {
    ((item_count.min(10) + 2) as u16).max(3)
}

/// 【模糊选择】【截断文本】按可见宽度裁剪含 ANSI 序列的字符串。
///
/// 参数:
/// - `value`: 原文
/// - `max`: 最大列数
///
/// 返回:
/// - 裁剪结果
fn truncate_display(value: &str, max: usize) -> String {
    crate::render::ansi_clip::clip_ansi_to_width(value, max, "…")
}

struct InlineRawMode {
    _mouse: super::mouse_capture::MouseCapture,
    stdout: io::Stdout,
    was_raw: bool,
}

impl InlineRawMode {
    /// 【模糊选择】【会话启动】获取鼠标捕获、开启括号粘贴与原始模式，返回自动恢复守卫。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 终端模式守卫
    fn start() -> Result<Self> {
        let mouse = super::mouse_capture::MouseCapture::acquire()?;
        let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
        if !was_raw {
            terminal::enable_raw_mode()?;
        }
        let mut stdout = io::stdout();
        let _ = execute!(stdout, EnableBracketedPaste);
        Ok(Self {
            _mouse: mouse,
            stdout,
            was_raw,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FuzzyTarget {
    Item(usize),
    Key(KeyCode),
}

/// 【模糊选择】【底栏按钮】构造确认与取消按钮及其快捷键提示。
///
/// 参数:
/// - `width`: 可用列宽
///
/// 返回:
/// - 可点击的按钮行
fn fuzzy_buttons(width: usize) -> ButtonRow<FuzzyTarget> {
    ButtonRow::new(
        [
            (t("Select", "确认").into(), FuzzyTarget::Key(KeyCode::Enter)),
            (t("Cancel", "取消").into(), FuzzyTarget::Key(KeyCode::Esc)),
        ],
        width,
    )
    .with_hint(
        t(
            "Enter select · Esc cancel · ↑↓ choose",
            "Enter 确认 · Esc 取消 · ↑↓ 选择",
        ),
        width,
    )
}

impl Drop for InlineRawMode {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        // 仅恢复调用前状态：嵌套在 REPL raw mode 时不要关掉
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_candidate_row, inline_fuzzy_scroll_offset, sanitize_paste_query,
        should_process_inline_fuzzy_key,
    };
    use crate::render::panel_palette::{fg, PanelPalette};
    use crate::render::terminal_contrast::{contrast, MIN_TEXT_CONTRAST};
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

    /// 【模糊选择测试】【自适应对比度】验证深浅背景下选中行均保持箭头标记、无反色且对比度达标。
    #[test]
    fn selected_row_uses_arrow_without_reverse_video_and_adapts_to_light_theme() {
        for bg in [(18, 20, 24), (248, 249, 250), (194, 213, 231)] {
            let palette = PanelPalette::for_background(bg);
            let line = format_candidate_row(palette, "item", true, 40);
            assert!(line.contains('→'));
            assert!(!line.contains("\x1b[7m"));
            assert!(line.contains(&fg(palette.accent)));
            assert!(contrast(palette.accent, bg) >= MIN_TEXT_CONTRAST);
            assert!(contrast(palette.muted, bg) >= MIN_TEXT_CONTRAST);
        }
    }

    /// 【模糊选择测试】【粘贴净化】验证粘贴内容自动剔除回车换行与控制序列。
    #[test]
    fn paste_sanitizes_newlines_and_control_sequences() {
        let cleaned = sanitize_paste_query("gpt-5\r\n\x1b[31m\tmini\x07");
        assert_eq!(cleaned, "gpt-5 mini");
        assert!(!cleaned.contains('\n') && !cleaned.contains('\r') && !cleaned.contains('\x1b'));
    }
}
