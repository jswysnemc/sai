use super::style::{
    CODE_COMMENT_STYLE, CODE_FUNCTION_STYLE, CODE_KEYWORD_STYLE, CODE_NUMBER_STYLE,
    CODE_STRING_STYLE, CODE_TOKEN_RESET, MD_CODE_LANG_STYLE, PRIMARY_STYLE, RESET,
};

/// 渲染不带横线的代码块标签行。
///
/// 标签弱化显示，与命令输出块的 dim gutter 属于同一视觉层。
///
/// 参数:
/// - `lang`: Markdown 代码块语言标识
///
/// 返回:
/// - 标签文本；空标签返回空文本
pub(crate) fn render_code_header(lang: &str) -> String {
    if lang.is_empty() {
        String::new()
    } else {
        format!("{MD_CODE_LANG_STYLE}{lang}{RESET}\n")
    }
}

/// 结束代码块但不渲染底部横线。
///
/// 参数:
/// - `_lines`: 代码块内容行，保留参数以兼容流式渲染调用
///
/// 返回:
/// - 空文本
pub(crate) fn render_code_footer(_lines: &[String]) -> String {
    String::new()
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
        // 行注释：仅限真正使用 // 注释的语言。shell 与 python 系没有这种注释，
        // 命令里的 http:// 会从 // 起被整段染成注释色
        if supports_line_comment(&lang)
            && index + 1 < chars.len()
            && chars[index] == '/'
            && chars[index + 1] == '/'
        {
            output.push_str(CODE_COMMENT_STYLE);
            output.extend(chars[index..].iter());
            output.push_str(CODE_TOKEN_RESET);
            return output;
        }
        // 块注释：CSS 只有 /* */ 一种注释，C 系语言也常用；行内闭合后继续高亮后文
        if supports_block_comment(&lang) && index + 1 < chars.len() && chars[index] == '/' && chars[index + 1] == '*' {
            let close = find_block_comment_end(&chars, index + 2);
            output.push_str(CODE_COMMENT_STYLE);
            output.extend(chars[index..close].iter());
            output.push_str(CODE_TOKEN_RESET);
            index = close;
            continue;
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
        "js" | "ts" | "tsx" | "jsx" | "javascript" | "typescript" => &[
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

/// 判断语言是否使用 `//` 行注释。
///
/// shell 与 python 系只有 `#` 注释；不加这层限制时，
/// 命令里的 `http://` 会从 `//` 起被整段染成注释色。
///
/// 参数:
/// - `lang`: 规范化后的语言标识
///
/// 返回:
/// - 使用 `//` 行注释时返回 true
fn supports_line_comment(lang: &str) -> bool {
    matches!(
        lang,
        "rs" | "rust" | "js" | "ts" | "tsx" | "jsx" | "javascript" | "typescript" | "c" | "cpp"
            | "java" | "go"
    )
}

/// 判断语言是否支持 `/* */` 块注释。
///
/// 参数:
/// - `lang`: 规范化后的语言标识
///
/// 返回:
/// - 支持块注释时返回 true
fn supports_block_comment(lang: &str) -> bool {
    matches!(
        lang,
        "css" | "rs" | "rust" | "js" | "ts" | "tsx" | "jsx" | "javascript" | "typescript" | "c"
            | "cpp" | "java" | "go"
    )
}

/// 查找块注释的结束位置（含结束定界符）。
///
/// 参数:
/// - `chars`: 当前行字符序列
/// - `start`: 注释正文起始下标
///
/// 返回:
/// - `*/` 之后的下标；行内未闭合时返回行尾
fn find_block_comment_end(chars: &[char], start: usize) -> usize {
    let mut index = start;
    while index + 1 < chars.len() {
        if chars[index] == '*' && chars[index + 1] == '/' {
            return index + 2;
        }
        index += 1;
    }
    chars.len()
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

    /// CSS 的块注释使用注释样式，行内闭合后继续高亮后文。
    #[test]
    fn css_block_comments_use_the_comment_style() {
        let output = highlight_code_line("css", "/* 页头 */ .page { gap: 1.5rem; }");

        assert!(output.starts_with(CODE_COMMENT_STYLE));
        assert!(output.contains("页头"));
        // 注释在行内闭合，后面的数值仍按数字着色
        assert!(output.contains(&format!("{CODE_NUMBER_STYLE}1.5rem")));
    }

    /// 行内未闭合的块注释一路着色到行尾。
    #[test]
    fn unclosed_block_comments_extend_to_the_end_of_line() {
        let output = highlight_code_line("css", "/* 卡片网格：默认三列");

        assert!(output.starts_with(CODE_COMMENT_STYLE));
        assert!(output.ends_with(CODE_TOKEN_RESET));
    }

    /// TypeScript 全名与缩写共享同一份关键字表。
    #[test]
    fn typescript_full_name_gets_keyword_highlighting() {
        let output = highlight_code_line("typescript", "const total = 1;");

        assert!(output.contains(&format!("{CODE_KEYWORD_STYLE}const")));
    }

    /// shell 命令里的 URL 不被当作 `//` 注释整段染色。
    #[test]
    fn shell_urls_are_not_treated_as_line_comments() {
        let output = highlight_code_line("bash", "curl http://127.0.0.1:8642/ && echo ok");

        assert!(!output.contains(CODE_COMMENT_STYLE));
    }

    /// rust 的 `//` 行注释仍然按注释着色。
    #[test]
    fn rust_line_comments_keep_the_comment_style() {
        let output = highlight_code_line("rust", "let a = 1; // 计数");

        assert!(output.contains(&format!("{CODE_COMMENT_STYLE}// 计数")));
    }
}
