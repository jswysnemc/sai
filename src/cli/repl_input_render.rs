use super::repl_chrome::{chrome_fixed_rows, ReplChrome};
use super::repl_clipboard::ReplClipboardState;
use super::repl_runtime::ReplRuntime;
use super::*;

/// 渲染极简 REPL 输入框与底栏。
///
/// 布局：
/// 1. 顶部分隔线
/// 2. 输入文本（无模式前缀）
/// 3. 底部分隔线
/// 4. 状态行：上下文占用 | 模型 · 思考等级
/// 5. 模式标签
/// 6. 可选斜杠补全提示
///
/// 参数:
/// - `stdout`: 终端输出
/// - `input_row`: 输入区起始行（可上移）
/// - `rendered_rows`: 上次渲染占用行数
/// - `chrome`: 底栏状态
/// - `input`: 当前输入
/// - `cursor`: 光标字符偏移
/// - `is_pasted`: 是否粘贴内容
/// - `runtime`: REPL 终端运行期
///
/// 返回:
/// - 渲染是否成功
pub(super) fn render_repl_input(
    stdout: &mut io::Stdout,
    input_row: &mut u16,
    rendered_rows: &mut u16,
    chrome: &ReplChrome,
    input: &str,
    cursor: usize,
    is_pasted: bool,
    clipboard_state: &ReplClipboardState,
    slash_selection: usize,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let (next_input_row, current_rows) = runtime.update_composer(
        chrome,
        input,
        cursor,
        is_pasted,
        clipboard_state.block_spans(input),
        slash_selection,
    )?;
    *input_row = next_input_row;
    runtime.draw_composer(stdout)?;
    *rendered_rows = current_rows;
    Ok(())
}

/// 输入框可见行的计算结果。
pub(super) struct VisibleInputLines {
    pub(super) lines: Vec<String>,
    /// 是否已收缩；布局层依赖该标志判定光标走原文还是折叠路径，
    /// 不能再用显示行数与原始行数相等来猜测（恰好 3 行时会误判）
    pub(super) collapsed: bool,
}

pub(super) fn repl_visible_input_lines(
    prefix: &str,
    lines: &[String],
    max_rows: u16,
    is_pasted: bool,
) -> VisibleInputLines {
    const LONG_PASTE_VISIBLE_CHARS: usize = 240;
    // 收缩后保留的首尾行最大字符数，保证收缩结果本身不会再撑高 composer
    const COLLAPSED_EDGE_CHARS: usize = 160;
    let total_rows = repl_prompt_rows(prefix, lines);
    let total_chars: usize = lines.iter().map(|line| line.chars().count()).sum();
    // 超过可见行数上限时无论内容来源一律收缩，否则手动多行输入会把
    // composer 顶出屏幕、底部区域相互覆盖；粘贴内容额外按字符阈值提前收缩
    let should_collapse = !lines.is_empty()
        && (total_rows > max_rows
            || (is_pasted
                && (total_chars > LONG_PASTE_VISIBLE_CHARS
                    || lines.iter().any(|line| line.chars().count() > 160))));
    if !should_collapse {
        return VisibleInputLines {
            lines: lines.to_vec(),
            collapsed: false,
        };
    }

    if lines.len() == 1 {
        let line = &lines[0];
        let chars = line.chars().count();
        let head: String = line.chars().take(48).collect();
        let omitted = chars.saturating_sub(head.chars().count());
        let note = if is_zh() {
            format!("... 已隐藏 {omitted} 字符输入内容 ...")
        } else {
            format!("... {omitted} input chars hidden ...")
        };
        return VisibleInputLines {
            lines: vec![format!("{head}…"), note],
            collapsed: true,
        };
    }

    let omitted_lines = lines.len().saturating_sub(2);
    let omitted = if is_zh() {
        format!("... 已隐藏 {omitted_lines} 行输入内容 ...")
    } else {
        format!("... {omitted_lines} input lines hidden ...")
    };
    VisibleInputLines {
        lines: vec![
            clip_collapsed_edge(&lines[0], COLLAPSED_EDGE_CHARS),
            omitted,
            clip_collapsed_edge(&lines[lines.len() - 1], COLLAPSED_EDGE_CHARS),
        ],
        collapsed: true,
    }
}

/// 截断收缩后保留的首尾行。
///
/// 参数:
/// - `line`: 原始行文本
/// - `max_chars`: 保留的最大字符数
///
/// 返回:
/// - 截断后的行；超长时以省略号结尾
fn clip_collapsed_edge(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let head: String = line.chars().take(max_chars).collect();
    format!("{head}…")
}

/// 清除 REPL 可编辑输入区。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `input_row`: 输入区起始行
/// - `rendered_rows`: 已渲染行数
///
/// 返回:
/// - 清除是否成功
pub(super) fn clear_repl_input(
    stdout: &mut io::Stdout,
    input_row: u16,
    rendered_rows: u16,
) -> Result<()> {
    for row_offset in 0..rendered_rows.max(1) {
        queue!(
            stdout,
            MoveTo(0, input_row.saturating_add(row_offset)),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stdout, MoveTo(0, input_row))?;
    stdout.flush()?;
    Ok(())
}

#[allow(dead_code)]
pub(super) fn repl_render_rows(prefix: &str, lines: &[String], has_suggestions: bool) -> u16 {
    chrome_fixed_rows()
        + repl_prompt_rows_for_cols(prefix, lines, terminal_cols())
        + u16::from(has_suggestions)
}

pub(super) fn repl_prompt_rows(prefix: &str, lines: &[String]) -> u16 {
    repl_prompt_rows_for_cols(prefix, lines, terminal_cols())
}

/// 为剪贴板原子块插入特殊颜色，保持原始文本和字符区间不变。
///
/// 参数:
/// - `line`: 待渲染的一行原始文本
/// - `line_start`: 该行在输入中的字符起点
/// - `spans`: 剪贴板原子块区间
///
/// 返回:
/// - 带 ANSI 样式的文本行
pub(super) fn style_clipboard_line(
    line: &str,
    line_start: usize,
    spans: &[super::repl_clipboard::ReplClipboardBlockSpan],
) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut active = None;
    for (offset, ch) in chars.iter().enumerate() {
        let position = line_start + offset;
        let next = spans
            .iter()
            .find(|span| span.start <= position && position < span.end)
            .map(|span| span.kind);
        if next != active {
            if active.is_some() {
                output.push_str("\x1b[0m");
            }
            if let Some(kind) = next {
                output.push_str(match kind {
                    super::repl_clipboard::ReplClipboardBlockKind::Text => {
                        "\x1b[48;5;25m\x1b[38;5;159m"
                    }
                    super::repl_clipboard::ReplClipboardBlockKind::Image => {
                        "\x1b[48;5;89m\x1b[38;5;225m"
                    }
                });
            }
            active = next;
        }
        output.push(*ch);
        if active.is_some() && spans.iter().any(|span| span.end == position + 1) {
            output.push_str("\x1b[0m");
            active = None;
        }
    }
    output
}

pub(super) fn repl_line_rows_for_cols(prefix: &str, line: &str, cols: usize) -> u16 {
    wrapped_line_rows(prefix, line, cols).min(u16::MAX as usize) as u16
}

pub(super) fn repl_prompt_rows_for_cols(prefix: &str, lines: &[String], cols: usize) -> u16 {
    let cols = cols.max(1);
    if lines.is_empty() {
        return 1;
    }
    let mut rows = 0usize;
    for (index, line) in lines.iter().enumerate() {
        rows += repl_line_rows_for_cols(if index == 0 { prefix } else { "" }, line, cols) as usize;
    }
    rows.max(1).min(u16::MAX as usize) as u16
}

pub(super) fn repl_cursor_position_for_cols(
    prefix: &str,
    input: &str,
    cursor: usize,
    cols: usize,
) -> (u16, u16) {
    let before_cursor = take_chars(input, cursor);
    let lines = repl_input_lines(&before_cursor);
    if lines.is_empty() {
        return (visible_width(prefix).min(u16::MAX as usize) as u16, 0);
    }
    // 行数与光标共用同一换行模拟器，宽字符跨界时两者才不会脱节
    let last_index = lines.len().saturating_sub(1);
    let mut row_offset = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let line_prefix = if index == 0 { prefix } else { "" };
        if index == last_index {
            let end = wrapped_end_position(line_prefix, line, cols);
            return (
                end.col.min(u16::MAX as usize) as u16,
                (row_offset + end.row).min(u16::MAX as usize) as u16,
            );
        }
        row_offset += wrapped_line_rows(line_prefix, line, cols);
    }
    (visible_width(prefix).min(u16::MAX as usize) as u16, 0)
}
