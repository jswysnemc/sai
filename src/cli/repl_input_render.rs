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

pub(super) fn repl_visible_input_lines(
    prefix: &str,
    lines: &[String],
    max_rows: u16,
    is_pasted: bool,
) -> Vec<String> {
    let total_rows = repl_prompt_rows(prefix, lines);
    if total_rows <= max_rows || lines.len() <= 2 || !is_pasted {
        return lines.to_vec();
    }

    let omitted_lines = lines.len().saturating_sub(2);
    let omitted = if is_zh() {
        format!("... 已隐藏 {omitted_lines} 行粘贴内容 ...")
    } else {
        format!("... {omitted_lines} pasted lines hidden ...")
    };
    vec![lines[0].clone(), omitted, lines[lines.len() - 1].clone()]
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
    let cols = cols.max(1);
    let width = visible_width(prefix) + visible_width(line);
    (width / cols + 1).min(u16::MAX as usize) as u16
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
    let cols = cols.max(1);
    let before_cursor = take_chars(input, cursor);
    let lines = repl_input_lines(&before_cursor);
    if lines.is_empty() {
        return (visible_width(prefix).min(u16::MAX as usize) as u16, 0);
    }
    let last_index = lines.len().saturating_sub(1);
    let mut row_offset = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let width = if index == 0 {
            visible_width(prefix) + visible_width(line)
        } else {
            visible_width(line)
        };
        if index == last_index {
            return (
                (width % cols).min(u16::MAX as usize) as u16,
                (row_offset + width / cols).min(u16::MAX as usize) as u16,
            );
        }
        row_offset += width / cols + 1;
    }
    (visible_width(prefix).min(u16::MAX as usize) as u16, 0)
}
