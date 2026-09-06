use crate::render::activity_animation::ACTIVITY_GUIDE;
use crossterm::terminal;
use unicode_width::UnicodeWidthChar;

/// 终端视觉引导线占用的列宽。
pub(crate) const GUIDE_COLUMN_WIDTH: usize = 1;
/// 正文与视觉引导线之间保留的空白列宽。
pub(crate) const GUIDE_CONTENT_GAP_WIDTH: usize = 1;
/// TUI 正文相对终端左边界的总缩进。
pub(crate) const CONTENT_LEFT_INDENT: usize = GUIDE_COLUMN_WIDTH + GUIDE_CONTENT_GAP_WIDTH;
/// diff 相对正文再向内收的列宽。
pub(crate) const DIFF_NESTED_INDENT: usize = 1;
/// diff 相对终端左右边界的总内收宽度。
pub(crate) const DIFF_BLOCK_INSET: usize = CONTENT_LEFT_INDENT + DIFF_NESTED_INDENT;

/// 判断字符是否为悬挂在视觉引导线列的行首符号。
///
/// `●` 用户回显、`•` 工具/状态、`◦` 思考、`✗` 失败、`›` 系统提示及活动竖条，
/// 都必须停在引导线列，否则会被推到正文列与正文混在一起。
///
/// 参数:
/// - `ch`: 行首首个可见字符
///
/// 返回:
/// - 是否保留在引导线列
fn is_guide_marker(ch: char) -> bool {
    matches!(
        ch,
        '●' | '○' | '•' | '◦' | '✗' | '›' | '◐' | '◓' | '◑' | '◒' | ACTIVITY_GUIDE
    )
}

/// 【终端】【视觉引导】统一历史与实时标题的圆形字形，保留原有状态颜色。
///
/// 参数: `text` 为待对齐的 ANSI 行
/// 返回: 首个引导符号经过归一化的行
pub(crate) fn normalize_guide_marker(text: &str) -> String {
    let Some(marker) = first_non_space_visible_char(text) else {
        return text.to_string();
    };
    let glyph = match marker.ch {
        '•' | '✗' => crate::render::style::GUIDE_FILLED,
        '◦' | '›' => crate::render::style::GUIDE_HOLLOW,
        _ => return text.to_string(),
    };
    format!("{}{glyph}{}", &text[..marker.start], &text[marker.end..])
}

/// 【终端】【视觉引导】替换活动标题的引导符号，正文与列宽保持不变。
///
/// 参数: `text` 为标题行，`frame` 为动画帧号
/// 返回: 带呼吸竖条的标题行
pub(crate) fn animate_guide_marker(text: &str, frame: usize) -> String {
    let Some(marker) =
        first_non_space_visible_char(text).filter(|marker| is_guide_marker(marker.ch))
    else {
        return text.to_string();
    };
    format!(
        "{}{}{}",
        &text[..marker.start],
        crate::render::activity_animation::render_activity_guide(frame),
        &text[marker.end..]
    )
}

/// 判断是否为会话 turn 水平分隔线（纯 `─`，可带 ANSI 弱化样式）。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的终端行
///
/// 返回:
/// - 是 turn 分隔线时为 true
fn is_turn_rule_line(text: &str) -> bool {
    let plain = crate::render::activity_animation::strip_ansi_for_test(text);
    // 仅顶格的通栏/正文净宽 turn 线。MD `---` 左右内收后更短，
    // 必须走普通正文缩进，不能被当成 turn 线拉回第 0 列。
    let trimmed_end = plain.trim_end();
    if trimmed_end.is_empty()
        || trimmed_end.starts_with(' ')
        || !trimmed_end.chars().all(|ch| ch == '─')
    {
        return false;
    }
    let n = trimmed_end.chars().count();
    let full = crate::render::markdown_blocks::horizontal_rule_width();
    // layout align 时 full=终端宽；cell 层 turn 线按正文净宽绘制
    let content = full.saturating_sub(CONTENT_LEFT_INDENT).max(1);
    n >= content && content >= 3
}

/// 将一行内容放到视觉引导线两侧的正确列。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的终端行
///
/// 返回:
/// - 引导符号位于左侧，普通正文位于右侧的终端行
pub(crate) fn align_to_guide_column(text: &str) -> String {
    let normalized = normalize_guide_marker(text);
    let text = normalized.as_str();
    let (leading_spaces, first_visible) = visible_line_start(text);
    if first_visible.is_some_and(is_guide_marker) {
        // 引导符必须顶格：diff 块缩进曾误伤标题行的 `•`，这里拉回第 0 列
        if leading_spaces == 0 {
            return text.to_string();
        }
        return remove_leading_visible_spaces(text, leading_spaces);
    }
    // turn 水平分隔线必须顶格通栏；按终端全宽重画，避免正文净宽线右侧留空
    if is_turn_rule_line(text) {
        let full = crate::render::markdown_blocks::horizontal_rule_width().max(1);
        return format!("\x1b[2m{}\x1b[0m", "─".repeat(full));
    }
    // diff 块内三类行（上下文、新增、删除）经 renderer 输出时结构已一致
    // （`行号 标记  正文`，上下文行的标记为空格），因此必须统一补同一宽度。
    // 按前导空格数推断层级会把上下文行的空格标记误判为额外缩进，使其正文
    // 相对增删行右移一列
    if text.contains("\x1b[K") {
        let indent = if has_diff_block_indent(text) {
            CONTENT_LEFT_INDENT
        } else {
            DIFF_BLOCK_INSET
        };
        return format!("{}{text}", " ".repeat(indent));
    }
    let indent = match leading_spaces {
        // 无缩进正文移动到引导线右侧
        0 => CONTENT_LEFT_INDENT,
        // 一列缩进表示 diff 内层，叠加正文基线后共三列
        1 => CONTENT_LEFT_INDENT,
        // cell 已经为续行预留正文基线，不重复添加
        _ => 0,
    };
    format!("{}{text}", " ".repeat(indent))
}

/// 【终端】【diff 对齐】判断 diff 行是否已带块内缩进。
///
/// 块缩进由 `indent_diff_for_transcript` 补在行首（样式序列之前），
/// 而 CLI 路径的裸 diff 行没有该缩进。只统计原始前导空格即可区分两者：
/// 增删行的行号填充位于样式序列之后，不会计入。
///
/// 参数:
/// - `text`: 已完成样式渲染的 diff 行
///
/// 返回:
/// - 已带块缩进时返回 true
fn has_diff_block_indent(text: &str) -> bool {
    // 块缩进补在样式序列之前，因此只统计首个样式序列之前的空格；
    // 上下文行没有样式前缀，其行首空格同时含块缩进与行号填充，
    // 故以是否达到块缩进宽度为准即可区分 CLI 裸行与 transcript 行
    let prefix_end = text.find('\x1b').unwrap_or(text.len());
    text[..prefix_end]
        .bytes()
        .take_while(|byte| *byte == b' ')
        .count()
        >= DIFF_NESTED_INDENT
}

/// 【终端】【响应式引导】按当前终端实际可用列数对齐视觉引导区。
///
/// 宽终端保留完整的“符号 + 间隔”两列；两列终端压缩为单个符号；
/// 单列终端移除引导符号，优先保留正文内容。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的终端行
/// - `guide_width`: 当前终端允许使用的视觉引导区列数
///
/// 返回:
/// - 已按实际引导区宽度压缩或对齐的终端行
pub(crate) fn align_to_guide_column_with_width(text: &str, guide_width: usize) -> String {
    // 【终端】【响应式引导】1. 先按完整两列引导区完成基础对齐
    let guide_width = guide_width.min(CONTENT_LEFT_INDENT);
    let aligned = align_to_guide_column(text);
    if guide_width == CONTENT_LEFT_INDENT {
        return aligned;
    }

    // 【终端】【响应式引导】2. 引导符号行压缩符号与正文之间的间隔
    let (_, first_visible) = visible_line_start(&aligned);
    if first_visible.is_some_and(is_guide_marker) {
        return compact_marker_guide(&aligned, guide_width);
    }
    // 【终端】【响应式引导】3. 普通正文与续行按目标引导区宽度移除前导空格
    remove_leading_visible_spaces(&aligned, CONTENT_LEFT_INDENT - guide_width)
}

/// 【终端】【CLI 布局】将流式输出块放到与 TUI 相同的视觉引导列。
///
/// 光标移动与清行序列继续从终端左边界执行；实际正文随后移动到引导列右侧，
/// `●` 和 `•` 等引导符号保留在左侧。
///
/// 参数:
/// - `text`: 可能包含 ANSI 光标控制序列的流式输出块
///
/// 返回:
/// - 已按视觉引导列对齐的终端文本
pub(crate) fn align_cli_stream_block(text: &str) -> String {
    text.split_inclusive('\n')
        .map(align_cli_stream_line)
        .collect()
}

/// 在 CLI 输出中显式软换行，确保续行重新进入正文列而不是落到引导线下方。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的文本块
///
/// 返回:
/// - 按当前终端正文宽度插入软换行后的文本
pub(crate) fn wrap_cli_stream_block(text: &str) -> String {
    wrap_cli_stream_block_with_width(text, cli_content_width())
}

/// 【终端】【CLI 换行】按指定正文宽度插入软换行。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的文本块
/// - `width`: 正文区域允许使用的列数
///
/// 返回:
/// - 按指定宽度插入软换行后的文本
fn wrap_cli_stream_block_with_width(text: &str, width: usize) -> String {
    let width = width.max(1);
    let mut output = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |body| (body, "\n"));
        let mut visible_width = 0usize;
        let mut index = 0usize;
        while index < body.len() {
            if body.as_bytes().get(index) == Some(&b'\x1b') {
                let end = crate::render::terminal_image::escape_sequence_end(body, index);
                if end > index {
                    output.push_str(&body[index..end]);
                    index = end;
                    continue;
                }
            }
            let ch = body[index..].chars().next().unwrap_or_default();
            let char_width = ch.width().unwrap_or(0);
            if char_width > 0 && visible_width > 0 && visible_width + char_width > width {
                output.push('\n');
                visible_width = 0;
            }
            output.push(ch);
            visible_width = visible_width.saturating_add(char_width);
            index += ch.len_utf8();
        }
        output.push_str(newline);
    }
    output
}

/// 【终端】【CLI 布局】对齐可能跨多个分片到达的纯文本正文。
///
/// 仅在物理行起点增加正文缩进，避免模型分片位于同一行中间时重复插入空格。
///
/// 参数:
/// - `text`: 当前纯文本增量
/// - `at_line_start`: 调用前是否位于物理行起点；调用后更新为最新状态
///
/// 返回:
/// - 已按视觉引导列对齐的纯文本增量
pub(crate) fn align_cli_text_delta(text: &str, at_line_start: &mut bool) -> String {
    let mut output = String::new();
    for segment in text.split_inclusive('\n') {
        let (body, newline) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |body| (body, "\n"));
        if *at_line_start && !body.is_empty() {
            output.push_str(&" ".repeat(CONTENT_LEFT_INDENT));
        }
        output.push_str(body);
        output.push_str(newline);
        *at_line_start = !newline.is_empty();
    }
    output
}

/// 【终端】【CLI 布局】返回 CLI 正文区域可使用的渲染宽度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 终端总宽度扣除引导列与间隔后的列数
pub(crate) fn cli_content_width() -> usize {
    terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100)
        .saturating_sub(CONTENT_LEFT_INDENT)
        .max(1)
}

/// 为 TUI diff 的每一行增加正文内部缩进。
///
/// 参数:
/// - `text`: 原始 diff 文本块
///
/// 返回:
/// - 每一行向正文内部再移动一列的文本
pub(crate) fn indent_diff_for_transcript(text: &str) -> String {
    indent_lines(text, DIFF_NESTED_INDENT)
}

/// 为 CLI diff 的每一行增加完整块级缩进。
///
/// 参数:
/// - `text`: 原始 diff 文本块
///
/// 返回:
/// - 每一行按 diff 总内收宽度移动后的文本
pub(crate) fn indent_diff_for_cli(text: &str) -> String {
    indent_lines(text, DIFF_BLOCK_INSET)
}

/// 生成清除终端最右侧 diff 边距的 ANSI 控制序列。
///
/// 该序列保存当前位置，移动到终端右边界，清除指定列数后恢复光标。
/// 不写入固定宽度空格，因此终端缩放时不会留下碎片色块。
///
/// 位置保存用 DECSC/DECRC（`ESC 7` / `ESC 8`）而不是 SCOSC/SCOR
///（`CSI s` / `CSI u`）：后者在部分 Windows 终端与复用器配置下被忽略，
/// 一旦保存/恢复失效，光标就停在右边界，后续输出从错误的列开始，
/// 整块 diff 向右错位。
///
/// 参数:
/// - `columns`: 需要恢复为终端默认背景的右侧列数
///
/// 返回:
/// - 清除右侧边距并恢复光标位置的 ANSI 序列
pub(crate) fn clear_right_margin(columns: usize) -> String {
    if columns == 0 {
        return String::new();
    }
    let move_left = columns.saturating_sub(1);
    format!("\x1b7\x1b[999C\x1b[{move_left}D\x1b[{columns}X\x1b8")
}

/// 为文本块的每一行增加指定列数的缩进。
///
/// 以 `•`/`◦` 等引导符开头的标题行保持顶格，只缩进 diff 正文色块。
///
/// 参数:
/// - `text`: 原始文本块
/// - `columns`: 缩进列数
///
/// 返回:
/// - 每一行增加缩进后的文本
fn indent_lines(text: &str, columns: usize) -> String {
    if columns == 0 {
        return text.to_string();
    }
    let indent = " ".repeat(columns);
    text.split_inclusive('\n')
        .map(|line| {
            let (content, newline) = match line.strip_suffix('\n') {
                Some(content) => (content, "\n"),
                None => (line, ""),
            };
            let (_, first) = visible_line_start(content);
            if first.is_some_and(is_guide_marker) {
                format!("{content}{newline}")
            } else {
                format!("{indent}{content}{newline}")
            }
        })
        .collect()
}

/// 单个 ANSI 可见字符的字节位置。
struct VisibleChar {
    ch: char,
    start: usize,
    end: usize,
}

/// 【终端】【ANSI 遍历】定位指定位置后的下一个可见字符。
///
/// 参数:
/// - `text`: 可能包含 ANSI 控制序列的终端行
/// - `index`: 开始检索的字节位置
///
/// 返回:
/// - 下一个可见字符及其字节范围；不存在时返回 None
fn next_visible_char(text: &str, mut index: usize) -> Option<VisibleChar> {
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(text, index)
                .max(index + ch.len_utf8());
            continue;
        }
        return Some(VisibleChar {
            ch,
            start: index,
            end: index + ch.len_utf8(),
        });
    }
    None
}

/// 【终端】【ANSI 遍历】读取行首空格数量与首个非空格可见字符。
///
/// 参数:
/// - `text`: ANSI 终端行
///
/// 返回:
/// - 行首可见空格数与首个非空格字符
fn visible_line_start(text: &str) -> (usize, Option<char>) {
    let mut index = 0usize;
    let mut leading_spaces = 0usize;
    while let Some(visible) = next_visible_char(text, index) {
        index = visible.end;
        if visible.ch == ' ' {
            leading_spaces += 1;
            continue;
        }
        return (leading_spaces, Some(visible.ch));
    }
    (leading_spaces, None)
}

/// 【终端】【响应式引导】将符号引导区压缩到指定列数。
///
/// 参数:
/// - `text`: 首个可见字符为引导符号的 ANSI 行
/// - `guide_width`: 目标引导区列数，仅接受零或一列
///
/// 返回:
/// - 单列时移除符号后的间隔，零列时移除符号及其样式前缀
fn compact_marker_guide(text: &str, guide_width: usize) -> String {
    let Some(marker) = first_non_space_visible_char(text) else {
        return text.to_string();
    };
    let gap = marker_gap_span(text, marker.end);
    match guide_width {
        0 => {
            let content_start = gap.map(|(_, end)| end).unwrap_or(marker.end);
            text[content_start..].to_string()
        }
        1 => gap.map_or_else(
            || text.to_string(),
            |(start, end)| format!("{}{}", &text[..start], &text[end..]),
        ),
        _ => text.to_string(),
    }
}

/// 【终端】【ANSI 遍历】定位首个非空格可见字符。
///
/// 参数:
/// - `text`: 可能包含 ANSI 控制序列的终端行
///
/// 返回:
/// - 首个非空格可见字符及其字节范围
fn first_non_space_visible_char(text: &str) -> Option<VisibleChar> {
    let mut index = 0usize;
    while let Some(visible) = next_visible_char(text, index) {
        index = visible.end;
        if visible.ch == ' ' {
            continue;
        }
        return Some(visible);
    }
    None
}

/// 【终端】【响应式引导】定位引导符号后的单列间隔。
///
/// 参数:
/// - `text`: 包含引导符号的 ANSI 行
/// - `marker_end`: 引导符号结束字节位置
///
/// 返回:
/// - 存在间隔时返回其起止字节位置
fn marker_gap_span(text: &str, marker_end: usize) -> Option<(usize, usize)> {
    let visible = next_visible_char(text, marker_end)?;
    (visible.ch == ' ').then_some((visible.start, visible.end))
}

/// 【终端】【响应式引导】移除行首指定数量的可见空格。
///
/// 参数:
/// - `text`: 已对齐到完整两列引导区的 ANSI 行
/// - `count`: 需要移除的可见空格数
///
/// 返回:
/// - 压缩后的 ANSI 行
fn remove_leading_visible_spaces(text: &str, count: usize) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0usize;
    let mut remaining = count;
    while remaining > 0 {
        let Some(visible) = next_visible_char(text, index) else {
            break;
        };
        output.push_str(&text[index..visible.start]);
        if visible.ch != ' ' {
            index = visible.start;
            break;
        }
        remaining -= 1;
        index = visible.end;
    }
    output.push_str(&text[index..]);
    output
}

/// 【终端】【CLI 布局】对齐单个物理行，并保留开头的光标控制序列。
///
/// 参数:
/// - `line`: 包含可选结尾换行的终端文本行
///
/// 返回:
/// - 光标控制仍位于行首、正文已经对齐的终端行
fn align_cli_stream_line(line: &str) -> String {
    let (body, newline) = line
        .strip_suffix('\n')
        .map_or((line, ""), |body| (body, "\n"));
    let control_end = leading_cursor_control_end(body);
    let (controls, content) = body.split_at(control_end);
    if content.is_empty() {
        return line.to_string();
    }
    format!("{controls}{}{newline}", align_to_guide_column(content))
}

/// 【终端】【CLI 布局】定位行首光标移动与清理序列的结束位置。
///
/// SGR 颜色序列属于正文样式，不在此剥离；终端图片协议同样作为正文处理。
///
/// 参数:
/// - `text`: 单个物理行
///
/// 返回:
/// - 行首控制序列后的字节偏移
fn leading_cursor_control_end(text: &str) -> usize {
    let mut index = 0usize;
    while index < text.len() {
        if text.as_bytes().get(index) == Some(&b'\r') {
            index += 1;
            continue;
        }
        if !text[index..].starts_with("\x1b[") {
            break;
        }
        let end = crate::render::terminal_image::escape_sequence_end(text, index);
        if end <= index || text.as_bytes().get(end.saturating_sub(1)) == Some(&b'm') {
            break;
        }
        index = end;
    }
    index
}

#[cfg(test)]
#[path = "content_indent_tests.rs"]
mod tests;
