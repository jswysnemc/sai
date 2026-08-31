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

/// 折行续行之间需要延续的行内高亮状态。
///
/// 长命令按终端宽度折成多个显示行后逐行高亮，若不延续未闭合的
/// 引号与注释上下文，续行会被从头重新 token 化：字符串中段的数字
/// 被染成数字色、普通词被染成命令色。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CodeLineHighlightState {
    /// 未闭合的引号字符（`"`、`'` 或反引号）
    open_quote: Option<char>,
    /// 未闭合字符串在行尾是否停在反斜杠转义中
    escaped: bool,
    /// 是否仍处于延续到逻辑行尾的行注释中
    in_comment: bool,
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
    let mut state = CodeLineHighlightState::default();
    highlight_code_line_continued(lang, line, &mut state)
}

/// 以显式跨行状态对单个显示行做语法高亮。
///
/// 供折行后的命令续行使用：引号与注释上下文经 `state` 在显示行之间
/// 传递。逻辑行边界调用方应重新使用默认状态。
///
/// 参数:
/// - `lang`: 语言标识
/// - `line`: 显示行文本
/// - `state`: 跨行高亮状态，本行扫描后原地更新
///
/// 返回:
/// - 带 ANSI 样式的代码行
pub(crate) fn highlight_code_line_continued(
    lang: &str,
    line: &str,
    state: &mut CodeLineHighlightState,
) -> String {
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
    // 0. 上一显示行的注释延续到本行（同一逻辑行内折行），整行按注释染色
    if state.in_comment {
        output.push_str(CODE_COMMENT_STYLE);
        output.extend(chars.iter());
        output.push_str(CODE_TOKEN_RESET);
        return output;
    }
    // 1. 上一显示行有未闭合字符串：从行首继续染到闭合引号为止
    if let Some(quote) = state.open_quote {
        let close = scan_string_end(&chars, 0, quote, state);
        output.push_str(CODE_STRING_STYLE);
        output.extend(chars[..close].iter());
        output.push_str(CODE_TOKEN_RESET);
        index = close;
    }
    while index < chars.len() {
        if let Some(marker) = comment_marker {
            if chars[index] == marker {
                output.push_str(CODE_COMMENT_STYLE);
                output.extend(chars[index..].iter());
                output.push_str(CODE_TOKEN_RESET);
                // 注释延续到逻辑行尾：折行的后续显示行仍在注释内
                state.in_comment = true;
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
        if supports_block_comment(&lang)
            && index + 1 < chars.len()
            && chars[index] == '/'
            && chars[index + 1] == '*'
        {
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
            let close = scan_string_end(&chars, index + 1, quote, state);
            output.push_str(CODE_STRING_STYLE);
            output.extend(chars[start..close].iter());
            output.push_str(CODE_TOKEN_RESET);
            index = close;
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

/// 从 `start` 扫描到闭合引号，返回扫描结束位置（含闭合引号）。
///
/// 行尾仍未闭合时，把引号与转义进度写回 `state`，供折行后的
/// 下一显示行继续按字符串染色。
///
/// 参数:
/// - `chars`: 当前行字符
/// - `start`: 引号内容起始下标
/// - `quote`: 引号字符
/// - `state`: 跨行高亮状态
///
/// 返回:
/// - 字符串结束（闭合引号之后）或行尾的下标
fn scan_string_end(
    chars: &[char],
    start: usize,
    quote: char,
    state: &mut CodeLineHighlightState,
) -> usize {
    let mut index = start;
    let mut escaped = state.escaped;
    state.escaped = false;
    while index < chars.len() {
        if escaped {
            escaped = false;
        } else if chars[index] == '\\' {
            escaped = true;
        } else if chars[index] == quote {
            state.open_quote = None;
            return index + 1;
        }
        index += 1;
    }
    state.open_quote = Some(quote);
    state.escaped = escaped;
    index
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
        // edit_diff::colors 已把这些扩展名映射到对应语言（因此 `//` 与
        // `/* */` 注释会被识别），但这里没有关键词表的话，func / class /
        // public 全都无着色，高亮看起来就很随意
        "go" => &[
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
        ],
        "java" => &[
            "abstract",
            "assert",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "do",
            "else",
            "enum",
            "extends",
            "final",
            "finally",
            "for",
            "if",
            "implements",
            "import",
            "instanceof",
            "interface",
            "native",
            "new",
            "package",
            "private",
            "protected",
            "public",
            "return",
            "static",
            "strictfp",
            "super",
            "switch",
            "synchronized",
            "this",
            "throw",
            "throws",
            "transient",
            "try",
            "void",
            "volatile",
            "while",
        ],
        "c" | "h" => &[
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long",
            "register", "restrict", "return", "short", "signed", "sizeof", "static", "struct",
            "switch", "typedef", "union", "unsigned", "void", "volatile", "while",
        ],
        "cpp" | "cc" | "hpp" | "cxx" => &[
            "auto",
            "bool",
            "break",
            "case",
            "catch",
            "char",
            "class",
            "const",
            "constexpr",
            "continue",
            "default",
            "delete",
            "do",
            "double",
            "else",
            "enum",
            "explicit",
            "extern",
            "false",
            "float",
            "for",
            "friend",
            "goto",
            "if",
            "inline",
            "int",
            "long",
            "namespace",
            "new",
            "nullptr",
            "operator",
            "private",
            "protected",
            "public",
            "return",
            "short",
            "signed",
            "sizeof",
            "static",
            "struct",
            "switch",
            "template",
            "this",
            "throw",
            "true",
            "try",
            "typedef",
            "typename",
            "union",
            "unsigned",
            "using",
            "virtual",
            "void",
            "volatile",
            "while",
        ],
        "sql" => &[
            "and", "as", "asc", "by", "create", "delete", "desc", "distinct", "drop", "from",
            "group", "having", "in", "insert", "into", "is", "join", "left", "like", "limit",
            "not", "null", "on", "or", "order", "right", "select", "set", "table", "update",
            "values", "where", "with",
        ],
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
        "rs" | "rust"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "javascript"
            | "typescript"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "hpp"
            | "cxx"
            | "java"
            | "go"
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
        "css"
            | "rs"
            | "rust"
            | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "javascript"
            | "typescript"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "hpp"
            | "cxx"
            | "java"
            | "go"
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

    /// 【终端】【命令着色】折行续行延续未闭合字符串，URL 中段不被重新 token 化。
    #[test]
    fn continued_lines_keep_open_string_state() {
        let mut state = CodeLineHighlightState::default();
        // 第一显示行：引号未闭合
        let first = highlight_code_line_continued("sh", r#"curl -o "https://cdn-"#, &mut state);
        assert!(first.contains(CODE_STRING_STYLE));
        // 第二显示行：整段仍是字符串，版本号不得染成数字色
        let second = highlight_code_line_continued(
            "sh",
            r#"zcode.z.ai/releases/3.7.6/x.deb" | tail -5"#,
            &mut state,
        );
        assert!(!second.contains(&format!("{CODE_NUMBER_STYLE}3.7.6")));
        assert!(second.starts_with(CODE_STRING_STYLE));
        // 引号闭合后恢复正常 token 化：tail 的 -5 属于字符串外内容
        assert!(second.contains("tail"));
        assert_eq!(state, CodeLineHighlightState::default());
    }

    /// 【终端】【命令着色】折行续行延续行注释状态到逻辑行尾。
    #[test]
    fn continued_lines_keep_comment_state() {
        let mut state = CodeLineHighlightState::default();
        let first = highlight_code_line_continued("sh", "echo hi # long comment that", &mut state);
        assert!(first.contains(CODE_COMMENT_STYLE));
        let second = highlight_code_line_continued("sh", "wraps to next display line", &mut state);
        assert!(second.starts_with(CODE_COMMENT_STYLE));
    }

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
