use super::style::{
    CODE_BLOCK_FRAME_STYLE, CODE_COMMENT_STYLE, CODE_FUNCTION_STYLE, CODE_KEYWORD_STYLE,
    CODE_NUMBER_STYLE, CODE_STRING_STYLE, CODE_TOKEN_RESET, PRIMARY_STYLE, RESET,
};
use super::table::visible_width;
use crossterm::terminal;

/// 计算代码块边框的最小宽度（基于终端宽度）。
pub(crate) fn frame_min_width() -> usize {
    terminal::size()
        .map(|(w, _)| usize::from(w) / 2)
        .unwrap_or(24)
        .clamp(20, 60)
}

/// 计算代码块边框的最大宽度（基于终端宽度）。
pub(crate) fn frame_max_width() -> usize {
    terminal::size()
        .map(|(w, _)| usize::from(w).max(1))
        .unwrap_or(120)
}

/// 按内容宽度、最小宽度和最大宽度计算代码块边框宽度。
///
/// 参数:
/// - `content_width`: 代码块内容的最大可见宽度
/// - `min_width`: 代码块边框最小宽度
/// - `max_width`: 代码块边框最大宽度
///
/// 返回:
/// - 不超过最大宽度的代码块边框宽度
fn frame_width_for_content(content_width: usize, min_width: usize, max_width: usize) -> usize {
    content_width.max(min_width).min(max_width.max(1))
}

/// 渲染代码块头部（开标签行）。
///
/// 流式渲染时在遇到开标签 ``` 即输出，宽度基于终端宽度，
/// 因为此时还不知道内容行的最长宽度。
///
/// 参数:
/// - `lang`: Markdown 代码块语言标识
///
/// 返回:
/// - 头部文本（含结尾换行）
pub(crate) fn render_code_header(lang: &str) -> String {
    let width = frame_width_for_content(0, frame_min_width(), frame_max_width());
    if lang.is_empty() {
        format!("{CODE_BLOCK_FRAME_STYLE}{}{RESET}\n", "─".repeat(width))
    } else {
        let prefix = format!("── {lang} ");
        let prefix_width = visible_width(&prefix);
        let padding = width.saturating_sub(prefix_width);
        format!(
            "{CODE_BLOCK_FRAME_STYLE}{prefix}{}{RESET}\n",
            "─".repeat(padding)
        )
    }
}

/// 渲染代码块尾部（闭标签行）。
///
/// 流式渲染时在遇到闭标签 ``` 或 flush 时输出，
/// 宽度取最长内容行与终端最小宽度的较大值（CJK 双宽）。
///
/// 参数:
/// - `lines`: 代码块内容行
///
/// 返回:
/// - 尾部文本（含结尾换行）
pub(crate) fn render_code_footer(lines: &[String]) -> String {
    let content_width = lines
        .iter()
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);

    let width = frame_width_for_content(content_width, frame_min_width(), frame_max_width());
    format!("{CODE_BLOCK_FRAME_STYLE}{}{RESET}\n", "─".repeat(width))
}

/// 对单行代码做轻量语法高亮。
///
/// 参数:
/// - `lang`: 语言标识
/// - `line`: 代码行
///
/// 返回:
/// - 带 ANSI 样式的代码行
pub(crate) fn highlight_code_line(lang: &str, line: &str) -> String {
    let lang = lang.trim().to_ascii_lowercase();
    if lang.is_empty() {
        return line.to_string();
    }
    let comment_marker = match lang.as_str() {
        "py" | "python" | "sh" | "bash" | "zsh" | "fish" | "toml" | "yaml" | "yml" => Some('#'),
        "rs" | "rust" | "js" | "ts" | "tsx" | "jsx" | "c" | "cpp" | "java" | "go" => None,
        _ => None,
    };
    let mut output = String::new();
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if let Some(marker) = comment_marker {
            if chars[index] == marker {
                output.push_str(CODE_COMMENT_STYLE);
                output.extend(chars[index..].iter());
                output.push_str(CODE_TOKEN_RESET);
                return output;
            }
        }
        if index + 1 < chars.len() && chars[index] == '/' && chars[index + 1] == '/' {
            output.push_str(CODE_COMMENT_STYLE);
            output.extend(chars[index..].iter());
            output.push_str(CODE_TOKEN_RESET);
            return output;
        }
        if chars[index] == '"'
            || chars[index] == '\''
            || (chars[index] == '`'
                && matches!(lang.as_str(), "js" | "ts" | "tsx" | "jsx" | "sh" | "bash"))
        {
            let quote = chars[index];
            let start = index;
            index += 1;
            let mut escaped = false;
            while index < chars.len() {
                if escaped {
                    escaped = false;
                } else if chars[index] == '\\' {
                    escaped = true;
                } else if chars[index] == quote {
                    index += 1;
                    break;
                }
                index += 1;
            }
            output.push_str(CODE_STRING_STYLE);
            output.extend(chars[start..index].iter());
            output.push_str(CODE_TOKEN_RESET);
            continue;
        }
        if chars[index].is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || matches!(chars[index], '_' | '.'))
            {
                index += 1;
            }
            output.push_str(CODE_NUMBER_STYLE);
            output.extend(chars[start..index].iter());
            output.push_str(CODE_TOKEN_RESET);
            continue;
        }
        if is_code_word_start(chars[index]) {
            let start = index;
            index += 1;
            while index < chars.len() && is_code_word_char(chars[index]) {
                index += 1;
            }
            let token = chars[start..index].iter().collect::<String>();
            let style = if code_keywords(&lang).contains(&token.as_str()) {
                Some(CODE_KEYWORD_STYLE)
            } else if matches!(
                token.as_str(),
                "true" | "false" | "null" | "None" | "Some" | "Ok" | "Err"
            ) {
                Some(CODE_NUMBER_STYLE)
            } else if next_non_space_is_open_paren(&chars, index) {
                Some(CODE_FUNCTION_STYLE)
            } else {
                None
            };
            if let Some(style) = style {
                output.push_str(style);
                output.push_str(&token);
                output.push_str(CODE_TOKEN_RESET);
            } else {
                output.push_str(PRIMARY_STYLE);
                output.push_str(&token);
                output.push_str(CODE_TOKEN_RESET);
            }
            continue;
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

/// 返回指定语言的关键词列表。
///
/// 参数:
/// - `lang`: 语言标识
///
/// 返回:
/// - 静态关键词数组
fn code_keywords(lang: &str) -> &'static [&'static str] {
    match lang {
        "rs" | "rust" => &[
            "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "trait", "type", "unsafe", "use",
            "where", "while",
        ],
        "py" | "python" => &[
            "and", "as", "async", "await", "break", "class", "continue", "def", "elif", "else",
            "except", "finally", "for", "from", "if", "import", "in", "is", "lambda", "not", "or",
            "pass", "raise", "return", "try", "while", "with", "yield",
        ],
        "js" | "ts" | "tsx" | "jsx" => &[
            "async", "await", "break", "case", "catch", "class", "const", "continue", "default",
            "else", "export", "extends", "finally", "for", "from", "function", "if", "import",
            "let", "new", "return", "switch", "throw", "try", "typeof", "var", "while",
        ],
        "sh" | "bash" | "zsh" | "fish" => &[
            "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
            "then", "while",
        ],
        "json" | "toml" | "yaml" | "yml" => &["true", "false", "null"],
        _ => &[],
    }
}

/// 判断字符是否可作为代码标识符起始。
///
/// 参数:
/// - `ch`: 待判断字符
///
/// 返回:
/// - 是否为标识符起始字符
fn is_code_word_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// 判断字符是否可作为代码标识符组成部分。
///
/// 参数:
/// - `ch`: 待判断字符
///
/// 返回:
/// - 是否为标识符组成字符
fn is_code_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// 判断下一个非空白字符是否为左括号。
///
/// 参数:
/// - `chars`: 字符数组
/// - `index`: 起始索引
///
/// 返回:
/// - 下一个非空白字符是否为 `(`
fn next_non_space_is_open_paren(chars: &[char], mut index: usize) -> bool {
    while index < chars.len() && chars[index].is_whitespace() {
        index += 1;
    }
    chars.get(index) == Some(&'(')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_width_does_not_exceed_terminal_limit() {
        assert_eq!(frame_width_for_content(200, 20, 80), 80);
    }

    #[test]
    fn frame_width_keeps_minimum_for_short_content() {
        assert_eq!(frame_width_for_content(8, 20, 80), 20);
    }
}
