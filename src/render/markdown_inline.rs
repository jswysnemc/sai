use crate::render::asset_block;
use crate::render::style::{
    BOLD_STYLE, IMAGE_STYLE, INLINE_CODE_STYLE, ITALIC_STYLE, LINK_LABEL_STYLE, RESET,
    STRIKE_STYLE, URL_STYLE,
};
use crate::render::table::CellContent;

/// 行内公式在当前输出表面中的渲染策略。
#[derive(Clone, Copy)]
pub(crate) enum InlineMathMode {
    TerminalImage,
    Source,
}

/// 渲染 Markdown 行内语法。
///
/// 参数:
/// - `text`: 原始行内文本
///
/// 返回:
/// - 带 ANSI 样式的行内文本
pub(crate) fn render_inline(text: &str) -> String {
    render_inline_with_math_mode(text, InlineMathMode::TerminalImage)
}

/// 按指定公式策略渲染 Markdown 行内语法。
///
/// 参数:
/// - `text`: 原始行内文本
/// - `math_mode`: 行内公式渲染策略
///
/// 返回:
/// - 带 ANSI 样式的行内文本
pub(crate) fn render_inline_with_math_mode(text: &str, math_mode: InlineMathMode) -> String {
    let mut output = String::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '!' && chars[index + 1] == '[' {
            if let Some(label_end) = find_marker(&chars, index + 2, ']') {
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(url_end) = find_marker(&chars, label_end + 2, ')') {
                        let alt = chars[index + 2..label_end].iter().collect::<String>();
                        output.push_str(IMAGE_STYLE);
                        output.push_str("[image");
                        if !alt.is_empty() {
                            output.push_str(": ");
                            output.push_str(&alt);
                        }
                        output.push(']');
                        output.push_str(RESET);
                        output.push('(');
                        output.push_str(&render_url(
                            &chars[label_end + 2..url_end].iter().collect::<String>(),
                        ));
                        output.push(')');
                        index = url_end + 1;
                        continue;
                    }
                }
            }
        }
        if chars[index] == '`' {
            if is_line_start_formula_prefix(&output, &chars, index) {
                index += 1;
                continue;
            }
            if let Some(end) = find_marker(&chars, index + 1, '`') {
                output.push_str(INLINE_CODE_STYLE);
                output.extend(chars[index + 1..end].iter());
                output.push_str(RESET);
                index = end + 1;
                continue;
            }
        }
        if chars[index] == '、' && is_line_start_formula_prefix(&output, &chars, index) {
            index += 1;
            continue;
        }
        if index + 1 < chars.len() && chars[index] == '$' && chars[index + 1] == '$' {
            if let Some(end) = find_double_marker(&chars, index + 2, '$') {
                let formula = chars[index + 2..end].iter().collect::<String>();
                match math_mode {
                    InlineMathMode::TerminalImage => {
                        output.push_str(&asset_block::render_inline_math(&formula));
                    }
                    InlineMathMode::Source => output.extend(chars[index..end + 2].iter()),
                }
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '$' {
            if let Some(end) = find_marker(&chars, index + 1, '$') {
                let formula = chars[index + 1..end].iter().collect::<String>();
                match math_mode {
                    InlineMathMode::TerminalImage => {
                        output.push_str(&asset_block::render_inline_math(&formula));
                    }
                    InlineMathMode::Source => output.extend(chars[index..=end].iter()),
                }
                index = end + 1;
                continue;
            }
        }
        if index + 1 < chars.len() && chars[index] == '~' && chars[index + 1] == '~' {
            if let Some(end) = find_double_marker(&chars, index + 2, '~') {
                output.push_str(STRIKE_STYLE);
                output.extend(chars[index + 2..end].iter());
                output.push_str(RESET);
                index = end + 2;
                continue;
            }
        }
        if index + 1 < chars.len() && chars[index] == '*' && chars[index + 1] == '*' {
            if let Some(end) = find_double_marker(&chars, index + 2, '*') {
                output.push_str(BOLD_STYLE);
                output.extend(chars[index + 2..end].iter());
                output.push_str(RESET);
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '*' {
            if let Some(end) = find_marker(&chars, index + 1, '*') {
                output.push_str(ITALIC_STYLE);
                output.extend(chars[index + 1..end].iter());
                output.push_str(RESET);
                index = end + 1;
                continue;
            }
        }
        if chars[index] == '_' {
            if is_emphasis_start(&chars, index) {
                if let Some(end) = find_emphasis_end(&chars, index + 1, '_') {
                    output.push_str(ITALIC_STYLE);
                    output.extend(chars[index + 1..end].iter());
                    output.push_str(RESET);
                    index = end + 1;
                    continue;
                }
            }
        }
        if chars[index] == '[' {
            if let Some(label_end) = find_marker(&chars, index + 1, ']') {
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(url_end) = find_marker(&chars, label_end + 2, ')') {
                        output.push_str(LINK_LABEL_STYLE);
                        output.extend(chars[index + 1..label_end].iter());
                        output.push_str(RESET);
                        output.push(' ');
                        output.push_str(&render_url_wrapped(
                            &chars[label_end + 2..url_end].iter().collect::<String>(),
                        ));
                        index = url_end + 1;
                        continue;
                    }
                }
            }
        }
        if chars[index] == '<' {
            if let Some(end) = find_marker(&chars, index + 1, '>') {
                let value = chars[index + 1..end].iter().collect::<String>();
                if value.starts_with("http://") || value.starts_with("https://") {
                    output.push_str("\x1b[4m");
                    output.push_str(&render_url_wrapped(&value));
                    output.push_str(RESET);
                    index = end + 1;
                    continue;
                }
                if let Some(rendered) = render_html_tag(&value) {
                    output.push_str(&rendered);
                    index = end + 1;
                    continue;
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// 渲染表格单元格内的 Markdown 行内语法。
///
/// 与 `render_inline` 不同，此函数保证输出为单行文本：
/// - 图片渲染为 `[image]` 占位符
/// - 数学公式渲染为终端半块图片
/// - 链接仅显示标签文本
/// - 列表和引用折叠为单行
///
/// 参数:
/// - `text`: 原始行内文本
///
/// 返回:
/// - 带 ANSI 样式的单行文本
pub(crate) fn render_table_cell(text: &str) -> String {
    let text = normalize_cell_text(text);
    let mut output = String::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '!' && chars[index + 1] == '[' {
            if let Some(label_end) = find_marker(&chars, index + 2, ']') {
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(url_end) = find_marker(&chars, label_end + 2, ')') {
                        output.push_str(IMAGE_STYLE);
                        output.push_str("[image]");
                        output.push_str(RESET);
                        index = url_end + 1;
                        continue;
                    }
                }
            }
        }
        if chars[index] == '`' {
            if let Some(end) = find_marker(&chars, index + 1, '`') {
                output.push_str(INLINE_CODE_STYLE);
                output.extend(chars[index + 1..end].iter());
                output.push_str(RESET);
                index = end + 1;
                continue;
            }
        }
        if index + 1 < chars.len() && chars[index] == '$' && chars[index + 1] == '$' {
            if let Some(end) = find_double_marker(&chars, index + 2, '$') {
                let formula = chars[index + 2..end].iter().collect::<String>();
                output.push_str(&asset_block::render_inline_math_halfblock(&formula));
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '$' {
            if let Some(end) = find_marker(&chars, index + 1, '$') {
                let formula = chars[index + 1..end].iter().collect::<String>();
                output.push_str(&asset_block::render_inline_math_halfblock(&formula));
                index = end + 1;
                continue;
            }
        }
        if index + 1 < chars.len() && chars[index] == '~' && chars[index + 1] == '~' {
            if let Some(end) = find_double_marker(&chars, index + 2, '~') {
                output.push_str(STRIKE_STYLE);
                output.extend(chars[index + 2..end].iter());
                output.push_str(RESET);
                index = end + 2;
                continue;
            }
        }
        if index + 1 < chars.len() && chars[index] == '*' && chars[index + 1] == '*' {
            if let Some(end) = find_double_marker(&chars, index + 2, '*') {
                output.push_str(BOLD_STYLE);
                output.extend(chars[index + 2..end].iter());
                output.push_str(RESET);
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '*' {
            if let Some(end) = find_marker(&chars, index + 1, '*') {
                output.push_str(ITALIC_STYLE);
                output.extend(chars[index + 1..end].iter());
                output.push_str(RESET);
                index = end + 1;
                continue;
            }
        }
        if chars[index] == '_' {
            if is_emphasis_start(&chars, index) {
                if let Some(end) = find_emphasis_end(&chars, index + 1, '_') {
                    output.push_str(ITALIC_STYLE);
                    output.extend(chars[index + 1..end].iter());
                    output.push_str(RESET);
                    index = end + 1;
                    continue;
                }
            }
        }
        if chars[index] == '[' {
            if let Some(label_end) = find_marker(&chars, index + 1, ']') {
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(url_end) = find_marker(&chars, label_end + 2, ')') {
                        output.push_str(LINK_LABEL_STYLE);
                        output.extend(chars[index + 1..label_end].iter());
                        output.push_str(RESET);
                        index = url_end + 1;
                        continue;
                    }
                }
            }
        }
        if chars[index] == '<' {
            if let Some(end) = find_marker(&chars, index + 1, '>') {
                let value = chars[index + 1..end].iter().collect::<String>();
                if value.starts_with("http://") || value.starts_with("https://") {
                    output.push_str(URL_STYLE);
                    output.push('<');
                    output.push_str(&value);
                    output.push('>');
                    output.push_str(RESET);
                    index = end + 1;
                    continue;
                }
                if let Some(rendered) = render_html_tag(&value) {
                    output.push_str(&rendered);
                    index = end + 1;
                    continue;
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// 渲染表格单元格，返回带显示宽度的单元格内容。
///
/// 只要单元格内含 `$...$` / `$$...$$` 公式（纯公式或文字+公式），
/// 整格都走终端图片协议（Kitty / iTerm2 / Sixel），不再用半块字符。
/// 无公式时走普通行内 Markdown 渲染。
///
/// 参数:
/// - `text`: 原始单元格文本
///
/// 返回:
/// - 表格单元格内容
pub(crate) fn render_table_cell_content(text: &str) -> CellContent {
    let text = normalize_cell_text(text);
    let trimmed = text.trim();
    if let Some((source, mixed)) = table_cell_math_render_source(trimmed) {
        return asset_block::render_inline_math_table_cell(&source, usize::MAX, mixed);
    }
    CellContent::from_inline(render_table_cell(&text))
}

/// 从表格单元格提取图片协议渲染源码。
///
/// 参数:
/// - `trimmed`: 已 trim 的单元格文本
///
/// 返回:
/// - `(渲染源码, 是否为文字+公式混合)`；无公式时返回空
fn table_cell_math_render_source(trimmed: &str) -> Option<(String, bool)> {
    if !trimmed.contains('$') {
        return None;
    }
    let dollar_count = trimmed.chars().filter(|&c| c == '$').count();
    // 1. 纯行内公式
    if dollar_count == 2 && trimmed.starts_with('$') && trimmed.ends_with('$') && trimmed.len() > 2
    {
        return Some((trimmed[1..trimmed.len() - 1].to_string(), false));
    }
    // 2. 纯显示公式
    if dollar_count == 4
        && trimmed.starts_with("$$")
        && trimmed.ends_with("$$")
        && trimmed.len() > 4
    {
        return Some((trimmed[2..trimmed.len() - 2].to_string(), false));
    }
    // 3. 文字 + 公式：整格作为混合源码（Typst/LaTeX 管线）
    if dollar_count >= 2 {
        return Some((trimmed.to_string(), true));
    }
    None
}

/// 预处理表格单元格文本，将多行内容折叠为单行。
///
/// - 列表项去除标记后用 ` · ` 连接
/// - 引用去除前缀后按层级连接
/// - 普通多行用空格连接
///
/// 参数:
/// - `text`: 原始单元格文本
///
/// 返回:
/// - 折叠后的单行文本
fn normalize_cell_text(text: &str) -> String {
    let has_newline = text.contains('\n');
    let has_br = text.contains("<br>") || text.contains("<br/>") || text.contains("<br />");
    let has_literal_newline = text.contains("\\n");
    if !has_newline && !has_br && !has_literal_newline {
        return text.to_string();
    }
    let text = text
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .replace("\\n", "\n");
    if !text.contains('\n') {
        return text;
    }
    let mut items: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            if !current.is_empty() {
                items.push(std::mem::take(&mut current));
            }
            current.push_str(rest);
        } else if trimmed.starts_with('>') {
            if !current.is_empty() {
                items.push(std::mem::take(&mut current));
            }
            let mut rest = trimmed;
            while let Some(stripped) = rest.strip_prefix("> ") {
                rest = stripped;
            }
            if rest == ">" {
                rest = "";
            }
            current.push_str(rest);
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
        }
    }
    if !current.is_empty() {
        items.push(current);
    }
    items.join(" · ")
}

/// 判断当前位置是否为独立公式行前的孤立前缀。
///
/// 参数:
/// - `output`: 已渲染输出
/// - `chars`: 原始字符数组
/// - `index`: 当前索引
///
/// 返回:
/// - 是否可清理此前缀
fn is_line_start_formula_prefix(output: &str, chars: &[char], index: usize) -> bool {
    output.trim().is_empty()
        && matches!(chars.get(index + 1), Some('$'))
        && chars[index + 1..].iter().filter(|ch| **ch == '$').count() >= 2
}

/// 渲染普通链接地址。
///
/// 参数:
/// - `url`: 链接地址
///
/// 返回:
/// - 带样式的链接地址
fn render_url(url: &str) -> String {
    format!("{URL_STYLE}{url}{RESET}")
}

/// 渲染带尖括号的链接地址。
///
/// 参数:
/// - `url`: 链接地址
///
/// 返回:
/// - 带尖括号的链接地址
fn render_url_wrapped(url: &str) -> String {
    format!("<{}>", render_url(url))
}

/// 渲染允许的 HTML 行内标签。
///
/// 参数:
/// - `tag`: 标签内容
///
/// 返回:
/// - 对应终端样式
fn render_html_tag(tag: &str) -> Option<String> {
    match tag.trim().to_ascii_lowercase().as_str() {
        "u" => Some("\x1b[4m".to_string()),
        "/u" => Some("\x1b[0m".to_string()),
        "sub" => Some("\x1b[2m".to_string()),
        "/sub" => Some("\x1b[0m".to_string()),
        "sup" => Some("\x1b[1m".to_string()),
        "/sup" => Some("\x1b[0m".to_string()),
        "br" | "br/" | "br /" => Some("\n".to_string()),
        _ => None,
    }
}

/// 查找单字符标记。
///
/// 参数:
/// - `chars`: 字符数组
/// - `start`: 起始索引
/// - `marker`: 标记字符
///
/// 返回:
/// - 命中的索引
fn find_marker(chars: &[char], start: usize, marker: char) -> Option<usize> {
    (start..chars.len()).find(|index| chars[*index] == marker)
}

/// 查找连续两个相同标记。
///
/// 参数:
/// - `chars`: 字符数组
/// - `start`: 起始索引
/// - `marker`: 标记字符
///
/// 返回:
/// - 第一个标记的索引
fn find_double_marker(chars: &[char], start: usize, marker: char) -> Option<usize> {
    (start..chars.len().saturating_sub(1))
        .find(|index| chars[*index] == marker && chars[index + 1] == marker)
}

/// 查找强调结束标记。
///
/// 参数:
/// - `chars`: 字符数组
/// - `start`: 起始索引
/// - `marker`: 标记字符
///
/// 返回:
/// - 命中的索引
fn find_emphasis_end(chars: &[char], start: usize, marker: char) -> Option<usize> {
    (start..chars.len()).find(|index| chars[*index] == marker && is_emphasis_end(chars, *index))
}

/// 判断下划线是否可作为强调起点。
///
/// 参数:
/// - `chars`: 字符数组
/// - `index`: 当前索引
///
/// 返回:
/// - 是否为强调起点
fn is_emphasis_start(chars: &[char], index: usize) -> bool {
    !chars
        .get(index.wrapping_sub(1))
        .is_some_and(|ch| is_word_char(*ch))
        && chars
            .get(index + 1)
            .is_some_and(|ch| !ch.is_whitespace() && *ch != '_')
}

/// 判断下划线是否可作为强调终点。
///
/// 参数:
/// - `chars`: 字符数组
/// - `index`: 当前索引
///
/// 返回:
/// - 是否为强调终点
fn is_emphasis_end(chars: &[char], index: usize) -> bool {
    chars
        .get(index.wrapping_sub(1))
        .is_some_and(|ch| !ch.is_whitespace() && *ch != '_')
        && !chars.get(index + 1).is_some_and(|ch| is_word_char(*ch))
}

/// 判断字符是否为英文单词字符。
///
/// 参数:
/// - `ch`: 待判断字符
///
/// 返回:
/// - 是否为英文单词字符
fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}
