//! syntect 代码着色。
//!
//! 终端按作用域映射到现有 ANSI 色，网页输出现有样式表能识别的 class。

use std::sync::OnceLock;

use syntect::parsing::{ParseState, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

use super::style::{
    CODE_COMMENT_STYLE, CODE_FUNCTION_STYLE, CODE_KEYWORD_STYLE, CODE_NUMBER_STYLE,
    CODE_STRING_STYLE, CODE_TOKEN_RESET,
};

/// 关闭后终端着色退回原来的词法扫描。
const FEATURE_SYNTECT_HIGHLIGHT: bool = true;

/// 单次着色的源码长度上限，避免超大文本占住解析器。
const MAX_HIGHLIGHT_CHARS: usize = 200_000;

/// 跨显示行保留的 syntect 解析状态。
pub(crate) struct Carry {
    lang: String,
    parse: ParseState,
    stack: ScopeStack,
    /// 行注释被折到下一显示行时，后续片段仍按注释着色
    carry_comment: bool,
}

/// 用 syntect 给一个显示行着色。
///
/// 参数:
/// - `lang`: 语言标识
/// - `line`: 显示行文本
/// - `carry`: 跨行解析状态
///
/// 返回:
/// - 带 ANSI 样式的行；语言无法识别或功能关闭时返回 None
pub(crate) fn paint(lang: &str, line: &str, carry: &mut Option<Box<Carry>>) -> Option<String> {
    if !FEATURE_SYNTECT_HIGHLIGHT {
        return None;
    }
    let lang = lang.trim().to_ascii_lowercase();
    if lang.is_empty() {
        return None;
    }
    let syntax = find_syntax(&lang)?;
    let session = session_for(carry, &lang, syntax);
    // 1. 折行切断的行注释没有换行符，解析器会把它当成已经结束
    if session.carry_comment {
        return Some(paint_span(CODE_COMMENT_STYLE, line));
    }
    let ops = session.parse.parse_line(line, syntaxes()).ok()?;
    Some(paint_ops_ansi(line, ops, session))
}

/// 把整段源码着色成 HTML。
///
/// 参数:
/// - `source`: 源码
/// - `language`: 语言标识
///
/// 返回:
/// - 安全 HTML，以及规范化后的语言标识
pub(crate) fn highlight_html(source: &str, language: &str) -> (String, String) {
    let lang = normalize_language(language);
    if !FEATURE_SYNTECT_HIGHLIGHT || source.chars().count() > MAX_HIGHLIGHT_CHARS {
        return (escape_html(source), lang);
    }
    let Some(syntax) = find_syntax(&lang) else {
        return (escape_html(source), lang);
    };
    let mut parse = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut html = String::new();
    // 2. 整段共用一个解析状态，块注释和多行字符串可以跨行
    for (index, raw) in source.split('\n').enumerate() {
        if index > 0 {
            html.push('\n');
        }
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let Ok(ops) = parse.parse_line(line, syntaxes()) else {
            html.push_str(&escape_html(line));
            continue;
        };
        html.push_str(&paint_ops_html(line, ops, &mut stack));
    }
    (html, lang)
}

/// 规范化语言标识，供网页缓存键和语法查找共用。
///
/// 参数:
/// - `language`: 调用方传入的语言或扩展名
///
/// 返回:
/// - 小写语言标识；无法识别时仍返回清洗后的原文
pub(crate) fn normalize_language(language: &str) -> String {
    let lang = language.trim().to_ascii_lowercase();
    extension_for(&lang)
        .unwrap_or(if lang.is_empty() { "" } else { &lang })
        .to_string()
}

/// 取得或重置当前语言的解析会话。
///
/// 参数:
/// - `carry`: 调用方保存的会话
/// - `lang`: 规范化语言
/// - `syntax`: 对应语法
///
/// 返回:
/// - 可继续解析的会话
fn session_for<'a>(
    carry: &'a mut Option<Box<Carry>>,
    lang: &str,
    syntax: &SyntaxReference,
) -> &'a mut Carry {
    let reset = carry.as_ref().is_none_or(|item| item.lang != lang);
    if reset {
        *carry = Some(Box::new(Carry {
            lang: lang.to_string(),
            parse: ParseState::new(syntax),
            stack: ScopeStack::new(),
            carry_comment: false,
        }));
    }
    carry.as_mut().expect("syntect session")
}

/// 按作用域操作给一行套上 ANSI 颜色。
///
/// 参数:
/// - `line`: 显示行
/// - `ops`: syntect 解析操作
/// - `session`: 当前会话，作用域栈会被更新
///
/// 返回:
/// - 带 ANSI 样式的行
fn paint_ops_ansi(line: &str, ops: Vec<(usize, ScopeStackOp)>, session: &mut Carry) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    let mut open_style = "";
    let mut ended_as_comment = false;
    for (index, op) in ops {
        if index > cursor {
            let style = ansi_style(&session.stack);
            ended_as_comment = style == CODE_COMMENT_STYLE;
            push_ansi(
                &mut output,
                &mut open_style,
                style,
                slice_line(line, cursor, index),
            );
            cursor = index;
        }
        let _ = session.stack.apply(&op);
    }
    if cursor < line.len() {
        let style = ansi_style(&session.stack);
        ended_as_comment = style == CODE_COMMENT_STYLE;
        push_ansi(
            &mut output,
            &mut open_style,
            style,
            slice_line(line, cursor, line.len()),
        );
    }
    if !open_style.is_empty() {
        output.push_str(CODE_TOKEN_RESET);
    }
    // 3. 行注释在逻辑行结束时会被解析器弹出；折行后续片段要自己续上
    session.carry_comment = ended_as_comment && !stack_has_comment(&session.stack);
    output
}

/// 按作用域操作给一行套上 HTML class。
///
/// 参数:
/// - `line`: 源码行
/// - `ops`: syntect 解析操作
/// - `stack`: 跨行作用域栈
///
/// 返回:
/// - 该行的安全 HTML
fn paint_ops_html(line: &str, ops: Vec<(usize, ScopeStackOp)>, stack: &mut ScopeStack) -> String {
    let mut html = String::new();
    let mut cursor = 0;
    for (index, op) in ops {
        if index > cursor {
            push_html(
                &mut html,
                html_class(stack),
                slice_line(line, cursor, index),
            );
            cursor = index;
        }
        let _ = stack.apply(&op);
    }
    if cursor < line.len() {
        push_html(
            &mut html,
            html_class(stack),
            slice_line(line, cursor, line.len()),
        );
    }
    html
}

/// 把作用域栈映射到终端颜色。
///
/// 参数:
/// - `stack`: 当前位置的作用域栈
///
/// 返回:
/// - ANSI 前缀；普通文本为空字符串
fn ansi_style(stack: &ScopeStack) -> &'static str {
    for scope in stack.as_slice().iter().rev() {
        let name = scope.to_string();
        if let Some(style) = style_for_scope(&name) {
            return style;
        }
    }
    ""
}

/// 把作用域栈映射到网页 class。
///
/// 参数:
/// - `stack`: 当前位置的作用域栈
///
/// 返回:
/// - class 名；不需要着色时为空字符串
fn html_class(stack: &ScopeStack) -> &'static str {
    match ansi_style(stack) {
        style if style == CODE_COMMENT_STYLE => "hljs-comment",
        style if style == CODE_STRING_STYLE => "hljs-string",
        style if style == CODE_KEYWORD_STYLE => "hljs-keyword",
        style if style == CODE_NUMBER_STYLE => "hljs-number",
        style if style == CODE_FUNCTION_STYLE => "hljs-title",
        _ => "",
    }
}

/// 判断作用域是否对应一种已有代码色。
///
/// 参数:
/// - `scope`: syntect 作用域名
///
/// 返回:
/// - 对应的 ANSI 前缀
fn style_for_scope(scope: &str) -> Option<&'static str> {
    // 注释定界符常挂在 punctuation.definition.comment，不能只看前缀
    if scope.split('.').any(|part| part == "comment") {
        Some(CODE_COMMENT_STYLE)
    } else if scope.starts_with("string") || scope.starts_with("constant.character") {
        Some(CODE_STRING_STYLE)
    } else if scope.split('.').any(|part| part == "unit")
        || scope.starts_with("constant.numeric")
        || scope.starts_with("constant.language")
    {
        Some(CODE_NUMBER_STYLE)
    } else if scope.starts_with("keyword")
        || scope.starts_with("storage.type")
        || scope.starts_with("storage.modifier")
    {
        Some(CODE_KEYWORD_STYLE)
    } else if scope.starts_with("entity.name.function")
        || scope.starts_with("support.function")
        || scope.starts_with("variable.function")
    {
        Some(CODE_FUNCTION_STYLE)
    } else {
        None
    }
}

/// 作用域栈里是否还留着注释。
///
/// 参数:
/// - `stack`: 行尾作用域栈
///
/// 返回:
/// - 块注释尚未闭合时为 true
fn stack_has_comment(stack: &ScopeStack) -> bool {
    stack
        .as_slice()
        .iter()
        .any(|scope| scope.to_string().starts_with("comment"))
}

/// 给整段文本套上同一种 ANSI 颜色。
///
/// 参数:
/// - `style`: ANSI 前缀
/// - `text`: 文本
///
/// 返回:
/// - 着色后的文本
fn paint_span(style: &'static str, text: &str) -> String {
    let mut output = String::new();
    let mut open_style = "";
    push_ansi(&mut output, &mut open_style, style, text);
    if !open_style.is_empty() {
        output.push_str(CODE_TOKEN_RESET);
    }
    output
}

/// 写入一段 ANSI 着色文本，相邻的同色片段合并。
///
/// 参数:
/// - `output`: 输出缓冲
/// - `open_style`: 当前尚未复位的颜色
/// - `style`: 本片段颜色，空字符串表示不着色
/// - `text`: 原文片段
///
/// 返回:
/// - 无
fn push_ansi(output: &mut String, open_style: &mut &'static str, style: &'static str, text: &str) {
    if text.is_empty() {
        return;
    }
    if style == *open_style {
        output.push_str(text);
        return;
    }
    if !open_style.is_empty() {
        output.push_str(CODE_TOKEN_RESET);
    }
    if !style.is_empty() {
        output.push_str(style);
    }
    output.push_str(text);
    *open_style = style;
}

/// 写入一段转义后的 HTML。
///
/// 参数:
/// - `html`: 输出缓冲
/// - `class`: span class，空字符串表示纯文本
/// - `text`: 原文片段
///
/// 返回:
/// - 无
fn push_html(html: &mut String, class: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    if class.is_empty() {
        html.push_str(&escape_html(text));
        return;
    }
    html.push_str("<span class=\"");
    html.push_str(class);
    html.push_str("\">");
    html.push_str(&escape_html(text));
    html.push_str("</span>");
}

/// 按字节下标切片，下标不落在字符边界时返回空串。
///
/// 参数:
/// - `line`: 原文
/// - `start`: 起始字节
/// - `end`: 结束字节
///
/// 返回:
/// - 对应片段
fn slice_line(line: &str, start: usize, end: usize) -> &str {
    line.get(start..end).unwrap_or("")
}

/// 转义 HTML 文本。
///
/// 参数:
/// - `text`: 原文
///
/// 返回:
/// - 可放进 HTML 文本节点的字符串
fn escape_html(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}

/// 加载内置语法，进程内只解析一次。
///
/// 返回:
/// - 语法集合
fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    // two-face 在 syntect 默认语法之外带有 TypeScript、TOML 等
    SET.get_or_init(two_face::syntax::extra_newlines)
}

/// 按语言标识查找语法。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 语法定义
fn find_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    find_mapped_syntax(lang).or_else(|| find_syntax_from_pack(lang))
}

/// 把语言别名对应到语法显示名。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - syntect 语法名
fn syntax_name(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "rs" | "rust" => "Rust",
        "py" | "python" => "Python",
        "js" | "javascript" | "jsx" | "cjs" | "mjs" => "JavaScript",
        "ts" | "typescript" | "tsx" => "TypeScript",
        "sh" | "bash" | "zsh" | "shell" => "Bourne Again Shell (bash)",
        "css" => "CSS",
        "go" => "Go",
        "java" => "Java",
        "c" | "h" => "C",
        "cpp" | "cc" | "hpp" | "cxx" | "c++" => "C++",
        "sql" => "SQL",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "html" => "HTML",
        "md" | "markdown" => "Markdown",
        "diff" | "patch" => "Diff",
        "ini" => "INI",
        _ => return None,
    })
}

/// 把项目里的语言别名收成 syntect 扩展名。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 扩展名；不支持时为 None
fn extension_for(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "rs" | "rust" => "rs",
        "py" | "python" => "py",
        "js" | "javascript" | "jsx" | "cjs" | "mjs" => "js",
        "ts" | "typescript" => "ts",
        "tsx" => "tsx",
        "sh" | "bash" | "zsh" | "shell" => "sh",
        "fish" => "fish",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "css" => "css",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" | "cxx" | "c++" => "cpp",
        "sql" => "sql",
        "md" | "markdown" => "md",
        "diff" | "patch" => "diff",
        "html" => "html",
        "ini" => "ini",
        _ => return None,
    })
}

/// 用已有别名定位语法。扩展名冲突时优先采用别名指定的语法名。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 语法定义；别名表没有这项时为 None
fn find_mapped_syntax(lang: &str) -> Option<&'static SyntaxReference> {
    let extension = extension_for(lang)?;
    let preferred = syntax_name(lang);
    let mut fallback = None;
    // 1. 同扩展名可能命中多套语法，名称与别名一致的优先
    for syntax in syntaxes().syntaxes().iter().filter(|syntax| !syntax.hidden) {
        let matched = syntax
            .file_extensions
            .iter()
            .any(|item| item.eq_ignore_ascii_case(extension));
        if !matched {
            continue;
        }
        if preferred.is_some_and(|name| syntax.name == name) {
            return Some(syntax);
        }
        if fallback.is_none() {
            fallback = Some(syntax);
        }
    }
    // 2. 扩展名没有命中时，退回别名里的语法名
    fallback.or_else(|| preferred.and_then(|name| syntaxes().find_syntax_by_name(name)))
}

/// 在整包语法里按名称或扩展名查找。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 语法定义；功能关闭或包内没有时为 None
fn find_syntax_from_pack(lang: &str) -> Option<&'static SyntaxReference> {
    if !FEATURE_SYNTECT_HIGHLIGHT || lang.is_empty() {
        return None;
    }
    // 1. 围栏里常见、但和语法名或扩展名不一致的写法
    if let Some(name) = pack_alias(lang) {
        if let Some(syntax) = syntaxes().find_syntax_by_name(name) {
            return Some(syntax);
        }
    }
    let set = syntaxes();
    // 2. 先用可见语法，避免内部语法抢走同名标识
    best_pack_match(set, lang, false).or_else(|| best_pack_match(set, lang, true))
}

/// 把围栏别名收成语法包里的显示名。
///
/// 参数:
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 语法显示名
fn pack_alias(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "csharp" => "C#",
        "fsharp" => "F#",
        "golang" => "Go",
        "plaintext" | "text" => "Plain Text",
        "docker" => "Dockerfile",
        "protobuf" => "Protocol Buffer",
        "objc" | "objectivec" => "Objective-C",
        "objcpp" => "Objective-C++",
        "viml" | "vimscript" => "VimL",
        "graphviz" => "Graphviz (DOT)",
        "gdscript" => "GDScript (Godot Engine)",
        _ => return None,
    })
}

/// 在可见或隐藏语法中取匹配等级最高的一项。
///
/// 参数:
/// - `set`: 语法集合
/// - `lang`: 小写语言标识
/// - `hidden`: 是否只看隐藏语法
///
/// 返回:
/// - 语法定义
fn best_pack_match<'a>(
    set: &'a SyntaxSet,
    lang: &str,
    hidden: bool,
) -> Option<&'a SyntaxReference> {
    let mut best: Option<(u8, &'a SyntaxReference)> = None;
    for syntax in set.syntaxes() {
        if syntax.hidden != hidden {
            continue;
        }
        let Some(rank) = pack_match_rank(syntax, lang) else {
            continue;
        };
        // 1. 等级相同保留先出现的语法，避免靠后的变体覆盖主语法
        if best.is_none_or(|(current, _)| rank < current) {
            best = Some((rank, syntax));
        }
    }
    best.map(|(_, syntax)| syntax)
}

/// 给语法和语言标识的匹配程度打分，分数越小越精确。
///
/// 参数:
/// - `syntax`: 候选语法
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 0 到 3；不匹配时为 None
fn pack_match_rank(syntax: &SyntaxReference, lang: &str) -> Option<u8> {
    if syntax.name.eq_ignore_ascii_case(lang) {
        return Some(0);
    }
    if syntax
        .file_extensions
        .first()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(lang))
    {
        return Some(1);
    }
    if syntax
        .file_extensions
        .iter()
        .any(|ext| ext.eq_ignore_ascii_case(lang))
    {
        return Some(2);
    }
    if name_has_token_prefix(&syntax.name, lang) {
        return Some(3);
    }
    None
}

/// 判断语法名是否以语言标识开头，并且后面是空格或括号。
///
/// 参数:
/// - `name`: 语法显示名
/// - `lang`: 小写语言标识
///
/// 返回:
/// - 是否为这种前缀
fn name_has_token_prefix(name: &str, lang: &str) -> bool {
    let mut name_chars = name.chars();
    for expected in lang.chars() {
        let Some(actual) = name_chars.next() else {
            return false;
        };
        if !actual.eq_ignore_ascii_case(&expected) {
            return false;
        }
    }
    matches!(name_chars.next(), Some(' ' | '('))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【代码着色】【网页】关键字进入现有 class，标签文本被转义。
    #[test]
    fn html_highlight_marks_keywords_and_escapes_tags() {
        let (html, lang) = highlight_html("const total = 1;\n<script>", "ts");
        assert_eq!(lang, "ts");
        assert!(
            html.contains("class=\"hljs-keyword\""),
            "html={html} syntax={:?}",
            find_syntax("ts").map(|item| item.name.as_str())
        );
        assert!(html.contains("const"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    /// 【代码着色】【网页】块注释跨行后仍保持注释 class。
    #[test]
    fn html_highlight_keeps_block_comments_across_lines() {
        let (html, _) = highlight_html("/* explanation\n new text\n */", "ts");
        assert!(html.contains("class=\"hljs-comment\""));
        assert!(html.contains("new text"));
    }

    /// 【代码着色】【语法包】每个可见语法都能用自己的名字找到。
    #[test]
    fn every_visible_syntax_is_reachable_by_its_name() {
        for syntax in syntaxes().syntaxes().iter().filter(|syntax| !syntax.hidden) {
            let token = syntax.name.to_ascii_lowercase();
            let found = find_syntax(&token).unwrap_or_else(|| panic!("missing {}", syntax.name));
            assert_eq!(found.name, syntax.name, "token={token}");
        }
    }

    /// 【代码着色】【语法包】扩展名和常见别名落到对应语法，原有别名保持不变。
    #[test]
    fn pack_extensions_and_aliases_resolve() {
        assert_eq!(find_syntax("kt").unwrap().name, "Kotlin");
        assert_eq!(find_syntax("dockerfile").unwrap().name, "Dockerfile");
        assert_eq!(find_syntax("csharp").unwrap().name, "C#");
        assert_eq!(find_syntax("golang").unwrap().name, "Go");
        assert_eq!(find_syntax("vue").unwrap().name, "Vue Component");
        assert_eq!(find_syntax("xml").unwrap().name, "XML");
        assert_eq!(find_syntax("svg").unwrap().name, "XML");
        assert_eq!(find_syntax("c").unwrap().name, "C");
        assert_eq!(find_syntax("h").unwrap().name, "C");
        assert_eq!(find_syntax("cpp").unwrap().name, "C++");
        assert_eq!(find_syntax("tsx").unwrap().name, "TypeScriptReact");
        assert_eq!(find_syntax("fish").unwrap().name, "Fish");
        assert_eq!(find_syntax("php").unwrap().name, "PHP");
        assert_eq!(find_syntax("graphviz").unwrap().name, "Graphviz (DOT)");
        let (html, _) = highlight_html("fun main() {}", "kotlin");
        assert!(html.contains("class=\"hljs-keyword\""), "{html}");
    }
}
