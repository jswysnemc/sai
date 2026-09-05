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
        Self::wrap_block_with_right_margin(text, width, 0)
    }

    /// 将 ANSI 文本块拆分到指定宽度，并清除带背景行的右侧边距。
    ///
    /// 参数:
    /// - `text`: 原始 ANSI 文本块
    /// - `width`: 已扣除右侧边距的内容宽度
    /// - `right_margin`: 终端右侧需要恢复默认背景的列数
    ///
    /// 返回:
    /// - 预换行并保留右侧背景边距的终端行
    pub(crate) fn wrap_block_with_right_margin(
        text: &str,
        width: usize,
        right_margin: usize,
    ) -> Vec<Self> {
        Self::wrap_block_with_right_margin_and_continuation_indent(text, width, right_margin, 0)
    }

    /// 【终端】【Diff 换行】拆分 ANSI 文本，并为自动续行恢复内部缩进。
    ///
    /// 右侧背景边距另行处理的调用方（CLI diff）使用此入口。
    ///
    /// 参数:
    /// - `text`: 原始 ANSI 文本块
    /// - `width`: 内容宽度
    /// - `continuation_indent`: 自动续行需要恢复的前导空格列数
    ///
    /// 返回:
    /// - 预换行并保留续行缩进的终端行
    pub(crate) fn wrap_block_with_continuation_indent(
        text: &str,
        width: usize,
        continuation_indent: usize,
    ) -> Vec<Self> {
        Self::wrap_block_with_right_margin_and_continuation_indent(
            text,
            width,
            0,
            continuation_indent,
        )
    }

    /// 【终端】【Diff 换行】拆分 ANSI 文本，并为自动续行恢复内部缩进。
    ///
    /// 参数:
    /// - `text`: 原始 ANSI 文本块
    /// - `width`: 已扣除右侧边距的内容宽度
    /// - `right_margin`: 终端右侧需要恢复默认背景的列数
    /// - `continuation_indent`: 自动续行需要恢复的前导空格列数
    ///
    /// 返回:
    /// - 预换行、保留续行缩进与右侧背景边距的终端行
    pub(crate) fn wrap_block_with_right_margin_and_continuation_indent(
        text: &str,
        width: usize,
        right_margin: usize,
        continuation_indent: usize,
    ) -> Vec<Self> {
        let mut lines = Vec::new();
        for raw_line in text.split('\n') {
            let plain = crate::render::activity_animation::strip_ansi_for_test(raw_line);
            let trimmed = plain.trim();
            // turn 横线：不短于目标宽度时重画到 width，避免烘焙通栏线被拆行。
            // MD 线更短（左右内收），必须原样保留，不能被拉成通栏。
            if trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '─') {
                let dash_in = trimmed.chars().filter(|ch| *ch == '─').count();
                let target = width.max(1);
                let keep_inset = dash_in < target;
                let dash_out = if keep_inset { dash_in } else { target };
                lines.push(Self::new(format!(
                    "\x1b[2m{}\x1b[0m",
                    "─".repeat(dash_out.max(1))
                )));
                continue;
            }
            lines.extend(wrap_line(
                raw_line,
                width,
                right_margin,
                continuation_indent,
            ));
        }
        lines
    }
}

/// 【终端】【ANSI 换行】按显示宽度切分单行，并恢复续行样式与缩进。
///
/// 参数:
/// - `text`: 单个 ANSI 物理行
/// - `width`: 当前内容区域列数
/// - `right_margin`: 终端右侧需要恢复默认背景的列数
/// - `continuation_indent`: 自动续行需要恢复的前导空格列数
///
/// 返回:
/// - 已按显示宽度拆分的终端行
fn wrap_line(
    text: &str,
    width: usize,
    right_margin: usize,
    continuation_indent: usize,
) -> Vec<AnsiLine> {
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
                    lines.push(finish_line(
                        &current,
                        width.saturating_sub(current_width),
                        fill_to_end,
                        &last_fill_sgr,
                        right_margin,
                    ));
                    (current, current_width) =
                        continuation_line(&active_sgr, width, continuation_indent);
                }
                current.push(' ');
                current_width += 1;
            }
            index += ch.len_utf8();
            continue;
        }

        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width > 0 && current_width.saturating_add(char_width) > width {
            lines.push(finish_line(
                &current,
                width.saturating_sub(current_width),
                fill_to_end,
                &last_fill_sgr,
                right_margin,
            ));
            (current, current_width) = continuation_line(&active_sgr, width, continuation_indent);
        }
        current.push(ch);
        current_width = current_width.saturating_add(char_width);
        index += ch.len_utf8();
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(finish_line(
            &current,
            width.saturating_sub(current_width),
            fill_to_end,
            &last_fill_sgr,
            right_margin,
        ));
    }
    lines
}

/// 【终端】【ANSI 换行】创建自动续行的初始内容与显示宽度。
///
/// 样式序列写在缩进空格之前：diff 增删行靠背景色连成一个矩形色块，
/// 若先输出裸空格再恢复背景，续行开头这几列会落在终端默认背景上，
/// 整块颜色就在每个续行的行首缺一个口子。
///
/// 参数:
/// - `active_sgr`: 上一行末尾仍然生效的 ANSI 样式
/// - `width`: 当前内容区域列数
/// - `continuation_indent`: 期望恢复的前导空格列数
///
/// 返回:
/// - `(续行初始文本, 已占用显示列数)`
fn continuation_line(
    active_sgr: &str,
    width: usize,
    continuation_indent: usize,
) -> (String, usize) {
    let indent = continuation_indent.min(width.saturating_sub(1));
    (format!("{active_sgr}{}", " ".repeat(indent)), indent)
}

/// 解析 SGR 操作码；参数为样式序列，返回跳过颜色分量后的操作码列表。
fn sgr_commands(sequence: &str) -> Vec<u16> {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|body| body.strip_suffix('m'))
    else {
        return Vec::new();
    };
    let mut values = params.split(';');
    let mut commands = Vec::new();
    while let Some(value) = values.next() {
        let code = value
            .split(':')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or("0");
        let Ok(code) = code.parse::<u16>() else {
            continue;
        };
        commands.push(code);
        // 1. 颜色分量中的 0 和 48 不是 reset 或背景设置；冒号格式的分量已在同一参数中
        if matches!(code, 38 | 48 | 58) && !value.contains(':') {
            let count = match values.next() {
                Some("2") => 3,
                Some("5") => 1,
                _ => 0,
            };
            for _ in 0..count {
                values.next();
            }
        }
    }
    commands
}

/// 判断样式是否重置全部属性；参数为 SGR 序列，返回是否包含独立 reset 操作。
fn is_reset_sgr(sequence: &str) -> bool {
    sgr_commands(sequence).contains(&0)
}

/// 判断样式是否设置背景；参数为 SGR 序列，返回是否包含背景色操作。
fn sgr_sets_background(sequence: &str) -> bool {
    sgr_commands(sequence)
        .into_iter()
        .any(|code| matches!(code, 40..=48 | 100..=107))
}

/// 更新续行样式；参数为当前样式缓冲和新序列，返回值为空。
fn update_active_sgr(active_sgr: &mut String, sequence: &str) {
    if is_reset_sgr(sequence) {
        active_sgr.clear();
    }
    if sequence != "\x1b[m" && sequence != "\x1b[0m" {
        active_sgr.push_str(sequence);
    }
}

/// 【终端】【差异背景】将背景补为真实空格，保证原生回滚区缩放后仍保留色块。
///
/// 参数: `text` 为已折行正文，`remaining` 为剩余列，`fill_to_end` 和 `fill_sgr` 指定背景，`right_margin` 为留白
/// 返回: 背景和右侧留白均已恢复的终端行
fn finish_line(
    text: &str,
    remaining: usize,
    fill_to_end: bool,
    fill_sgr: &str,
    right_margin: usize,
) -> AnsiLine {
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
        // 1. EL 产生的擦除空白在终端 reflow 时会丢失，写入真实空格保留背景
        output.push_str(&" ".repeat(remaining));
        output.push_str("\x1b[K");
    }
    output.push_str("\x1b[0m");
    if fill_to_end {
        output.push_str(&crate::render::content_indent::clear_right_margin(
            right_margin,
        ));
    }
    AnsiLine::new(output)
}

#[cfg(test)]
mod tests {
    use super::AnsiLine;

    /// 真实着色空格占满内容列，终端重排时不依赖擦除命令生成的空白。
    #[test]
    fn diff_background_padding_survives_scrollback_reflow() {
        let lines = AnsiLine::wrap_block_with_right_margin("\x1b[48;5;22m+ x\x1b[K\x1b[0m", 12, 3);
        let line = lines[0].as_str();
        let plain = crate::render::activity_animation::strip_ansi_for_test(line);
        assert_eq!(plain, "+ x         ");
        assert!(line.contains("         \x1b[K\x1b[0m"));
        assert!(line.ends_with(&crate::render::content_indent::clear_right_margin(3)));
    }

    /// 零值颜色分量不能清除续行背景，值为 48 的前景分量也不能被当作背景指令。
    #[test]
    fn diff_background_survives_rgb_and_palette_zero_components() {
        for background in ["\x1b[48;2;0;95;0m", "\x1b[48;5;0m", "\x1b[48:2::0:95:0m"] {
            let source = format!("{background}\x1b[38;2;48;0;0mabcdefgh\x1b[K\x1b[0m");
            let lines = AnsiLine::wrap_block(&source, 4);
            assert_eq!(lines.len(), 2);
            assert!(lines
                .iter()
                .all(|line| line.as_str().starts_with(background)));
        }
        assert!(!super::sgr_sets_background("\x1b[38;2;48;0;0m"));
        assert!(super::is_reset_sgr("\x1b[0;38;2;0;95;0m"));
    }

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

    /// diff 折行应在每个带背景的物理行末清除右侧边距。
    #[test]
    fn wrapped_background_lines_keep_right_margin() {
        let lines =
            AnsiLine::wrap_block_with_right_margin("\x1b[48;5;22m123456\x1b[K\x1b[0m", 3, 3);

        assert_eq!(lines.len(), 2);
        assert!(lines
            .iter()
            .all(|line| line.as_str().contains("\x1b[2D\x1b[3X")));
    }

    /// 【终端】【Diff 换行测试】验证带背景的自动续行恢复 diff 内部缩进。
    ///
    /// 缩进空格必须落在背景样式之后，否则续行开头这几列会用终端默认背景，
    /// 增删行的整块色块在每个续行的行首缺一个口子。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn wrapped_diff_lines_restore_continuation_indent() {
        let lines = AnsiLine::wrap_block_with_right_margin_and_continuation_indent(
            " \x1b[48;5;22m123456\x1b[K\x1b[0m",
            4,
            3,
            1,
        );

        assert_eq!(lines.len(), 2);
        // 续行先恢复背景再补缩进，缩进列因此也带上 diff 背景色
        assert!(lines[1].as_str().starts_with("\x1b[48;5;22m "));
        assert!(lines
            .iter()
            .all(|line| line.as_str().contains("\x1b[2D\x1b[3X")));
    }
}
