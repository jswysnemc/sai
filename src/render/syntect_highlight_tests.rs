use super::*;
use crate::render::style::PRIMARY_STYLE;

/// 【代码着色】【变量回归】未提供细分作用域的标识符仍使用主色，保留语法定义的其他颜色。
/// 参数: 无；返回: 无。
#[test]
fn ansi_highlight_preserves_unscoped_identifier_colors() {
    for language in ["rust", "rs"] {
        let mut carry = None;
        let output = paint(language, "let _msg2 = \"msg\"; // msg", &mut carry).unwrap();
        assert!(
            output.contains(&format!("{PRIMARY_STYLE}_msg2{CODE_TOKEN_RESET}")),
            "output={output:?}"
        );
        assert!(output.contains(CODE_KEYWORD_STYLE));
        assert!(
            output.contains(&format!("{CODE_STRING_STYLE}\"msg\"{CODE_TOKEN_RESET}")),
            "output={output:?}"
        );
        assert!(
            output.contains(&format!("{CODE_COMMENT_STYLE}// msg{CODE_TOKEN_RESET}")),
            "output={output:?}"
        );
    }
}

/// 【代码着色】【纯文本回归】纯文本语法原样返回，不插入 ANSI，切换回代码时重建解析状态。
/// 参数: 无；返回: 无。
#[test]
fn plain_text_stays_uncolored_when_switching_syntaxes() {
    for language in ["txt", "Plain Text"] {
        let mut carry = None;
        paint("rust", "let value = \"open string", &mut carry).unwrap();
        let source = "first.txt-DETAIL-0\n中文原文";
        assert_eq!(paint(language, source, &mut carry).unwrap(), source);
        let code = paint("rust", "let next = 42;", &mut carry).unwrap();
        assert!(
            code.contains(&format!("{CODE_KEYWORD_STYLE}let{CODE_TOKEN_RESET}")),
            "code={code:?}"
        );
    }
}

/// 【代码着色】【网页】关键字进入现有 class，标签文本被转义。
/// 参数: 无；返回: 无。
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
/// 参数: 无；返回: 无。
#[test]
fn html_highlight_keeps_block_comments_across_lines() {
    let (html, _) = highlight_html("/* explanation\n new text\n */", "ts");
    assert!(html.contains("class=\"hljs-comment\""));
    assert!(html.contains("new text"));
}

/// 【代码着色】【语法包】每个可见语法都能用自己的名字找到。
/// 参数: 无；返回: 无。
#[test]
fn every_visible_syntax_is_reachable_by_its_name() {
    for syntax in syntaxes().syntaxes().iter().filter(|syntax| !syntax.hidden) {
        let token = syntax.name.to_ascii_lowercase();
        let found = find_syntax(&token).unwrap_or_else(|| panic!("missing {}", syntax.name));
        assert_eq!(found.name, syntax.name, "token={token}");
    }
}

/// 【代码着色】【语法包】扩展名和常见别名落到对应语法，原有别名保持不变。
/// 参数: 无；返回: 无。
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
