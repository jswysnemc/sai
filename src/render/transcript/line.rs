use unicode_width::UnicodeWidthChar;

const TAB_STOP_COLUMNS: usize = 4;

/// 已按指定宽度预换行的 ANSI 终端行。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AnsiLine {
    text: String,
}

impl AnsiLine {
    /// 创建一条预换行 ANSI 终端行。
    ///
    /// 参数:
    /// - `text`: 不包含换行符的 ANSI 文本
    ///
    /// 返回:
    /// - ANSI 终端行
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }

    /// 返回可直接写入终端的 ANSI 文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 不包含换行符的 ANSI 文本
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    /// 将 ANSI 文本块拆分并预换行到指定终端宽度。
    ///
    /// 参数:
    /// - `text`: 原始 ANSI 文本块
    /// - `width`: 当前终端列数
    ///
    /// 返回:
    /// - 预换行后的终端行
    pub(crate) fn wrap_block(text: &str, width: usize) -> Vec<Self> {
        let mut lines = Vec::new();
        for raw_line in text.split('\n') {
            lines.extend(wrap_line(raw_line, width));
        }
        lines
    }
}

/// 按显示宽度切分单行 ANSI 文本，并在续行恢复活动样式。
fn wrap_line(text: &str, width: usize) -> Vec<AnsiLine> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut active_sgr = String::new();
    // 需要 EL 铺满背景时，记录最后一次背景相关 SGR，确保 \x1b[K 在 reset 之前生效
    let mut fill_to_end = text.contains("\x1b[K");
    let mut last_fill_sgr = String::new();
    let mut index = 0usize;

    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            let sequence = &text[index..end];
            match sequence.chars().last() {
                Some('m') => {
                    update_active_sgr(&mut active_sgr, sequence);
                    if sgr_sets_background(sequence) {
                        last_fill_sgr = active_sgr.clone();
                    }
                    // 原始 diff 行末尾的 reset 延后到 finish_line，避免 EL 在默认背景执行
                    if fill_to_end && is_reset_sgr(sequence) {
                        index = end.max(index + ch.len_utf8());
                        continue;
                    }
                    current.push_str(sequence);
                }
                Some('K') => {
                    fill_to_end = true;
                }
                _ => current.push_str(sequence),
            }
            index = end.max(index + ch.len_utf8());
            continue;
        }

        if ch == '\t' {
            let spaces = TAB_STOP_COLUMNS - (current_width % TAB_STOP_COLUMNS);
            // 1. 将制表符展开到固定四列制表位
            for _ in 0..spaces {
                // 2. 逐列折行，避免终端再次解释制表符后破坏 pager 行数
                if current_width >= width {
                    lines.push(finish_line(&current, fill_to_end, &last_fill_sgr));
                    current = active_sgr.clone();
                    current_width = 0;
                }
                current.push(' ');
                current_width += 1;
            }
            index += ch.len_utf8();
            continue;
        }

        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width > 0 && current_width.saturating_add(char_width) > width {
            lines.push(finish_line(&current, fill_to_end, &last_fill_sgr));
            current = active_sgr.clone();
            current_width = 0;
        }
        current.push(ch);
        current_width = current_width.saturating_add(char_width);
        index += ch.len_utf8();
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(finish_line(&current, fill_to_end, &last_fill_sgr));
    }
    lines
}

/// 判断 SGR 序列是否为 reset。
fn is_reset_sgr(sequence: &str) -> bool {
    let Some(body) = sequence.strip_prefix("\x1b[") else {
        return false;
    };
    let params = body.strip_suffix('m').unwrap_or(body);
    params.is_empty() || params.split(';').any(|value| value == "0")
}

/// 判断 SGR 是否设置背景色（48;...）。
fn sgr_sets_background(sequence: &str) -> bool {
    let Some(body) = sequence.strip_prefix("\x1b[") else {
        return false;
    };
    let params = body.strip_suffix('m').unwrap_or(body);
    params.split(';').any(|value| value == "48")
}

/// 更新续行需要恢复的 SGR 样式序列。
fn update_active_sgr(active_sgr: &mut String, sequence: &str) {
    let Some(body) = sequence.strip_prefix("\x1b[") else {
        return;
    };
    let params = body.strip_suffix('m').unwrap_or(body);
    let reset = params.is_empty() || params.split(';').any(|value| value == "0");
    if reset {
        active_sgr.clear();
    }
    if sequence != "\x1b[m" && sequence != "\x1b[0m" {
        active_sgr.push_str(sequence);
    }
}

/// 结束预换行行；EL 必须在 reset 之前、背景色仍生效时执行。
fn finish_line(text: &str, fill_to_end: bool, fill_sgr: &str) -> AnsiLine {
    let mut output = text.to_string();
    // 去掉可能残留的尾部 reset
    while output.ends_with("\x1b[0m") {
        output.truncate(output.len() - "\x1b[0m".len());
    }
    while output.ends_with("\x1b[m") {
        output.truncate(output.len() - "\x1b[m".len());
    }
    if fill_to_end {
        if !fill_sgr.is_empty() {
            output.push_str(fill_sgr);
        }
        output.push_str("\x1b[K");
    }
    output.push_str("\x1b[0m");
    AnsiLine::new(output)
}

#[cfg(test)]
mod tests {
    use super::AnsiLine;

    /// ANSI 文本中的制表符按四列制表位展开并参与折行。
    #[test]
    fn wraps_ansi_text_with_expanded_tab_stops() {
        let lines = AnsiLine::wrap_block("\x1b[31ma\tb\x1b[0m", 4);

        assert_eq!(lines.len(), 2);
        assert!(lines[0].as_str().contains("a   "));
        assert!(lines[1].as_str().contains("\x1b[31mb"));
        assert!(!lines.iter().any(|line| line.as_str().contains('\t')));
    }

    /// 窄终端会把一个制表符展开后的空格分配到多行。
    #[test]
    fn wraps_tab_expansion_across_narrow_rows() {
        let lines = AnsiLine::wrap_block("\tX", 2);

        assert_eq!(lines.len(), 3);
        assert!(lines[2].as_str().starts_with('X'));
    }
}
