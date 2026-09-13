use crate::cli::repl_text::char_terminal_width;
use crate::render::ansi_style::update_active_sgr;

/// 【终端】【输入折行】按输入框列宽切分文本，并在续行恢复原子块样式。
/// 参数: `text` 为含 ANSI 的逻辑行，`cols` 为正文列数
/// 返回: 包含满行光标占位行的独立视觉行
pub(super) fn wrap_styled_line(text: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    let mut index = 0usize;
    let mut active_style = String::new();
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            let sequence = &text[index..end];
            if sequence.ends_with('m') {
                update_active_sgr(&mut active_style, sequence);
            }
            current.push_str(sequence);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        if ch == '\t' {
            // 1. 制表位与光标测量共用八列规则，写出空格避免终端再次解释
            let next = ((width / 8 + 1) * 8).min(cols.saturating_sub(1));
            current.push_str(&" ".repeat(next.saturating_sub(width)));
            width = next;
            index += ch.len_utf8();
            continue;
        }
        let char_width = char_terminal_width(ch);
        if char_width > 0 && width + char_width > cols {
            // 2. 每行独立收束样式，下一行在正文前恢复有效颜色
            current.push_str("\x1b[0m");
            lines.push(current);
            current = active_style.clone();
            width = 0;
        }
        current.push(ch);
        width = width.saturating_add(char_width);
        index += ch.len_utf8();
    }
    current.push_str("\x1b[0m");
    lines.push(current);
    // 3. 恰好写满正文区域时，为光标单独保留下一行
    if width >= cols {
        lines.push(String::new());
    }
    lines
}
