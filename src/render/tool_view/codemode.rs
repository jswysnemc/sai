use crate::render::code_block::{highlight_code_line_continued, CodeLineHighlightState};
use serde_json::Value;

/// 【CodeMode】【代码着色】参数 arguments 为工具 JSON；返回保留缩进和跨行状态的 JavaScript 代码块。
pub(crate) fn render_arguments(arguments: &str) -> Option<String> {
    let arguments: Value = serde_json::from_str(arguments).ok()?;
    let code = arguments.get("code")?.as_str()?;
    let code = super::safe_text::plain(code);
    let mut state = CodeLineHighlightState::default();
    let mut output = String::from("\x1b[2mJavaScript\x1b[0m\n");
    for line in code.split_inclusive('\n') {
        output.push_str(&highlight_code_line_continued(
            "javascript",
            line,
            &mut state,
        ));
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【CodeMode】【着色回归】无参数；验证关键字、字符串着色且代码文本与缩进保持不变。
    #[test]
    fn javascript_keeps_source_and_highlights_tokens() {
        let source =
            "const result = await tools.run_command({command: 'printf hello'});\n  text(result);";
        let output = render_arguments(&serde_json::json!({"code":source}).to_string()).unwrap();
        assert!(output.matches('\u{1b}').count() > 4, "{output:?}");
        assert!(crate::render::activity_animation::strip_ansi_for_test(&output).ends_with(source));
    }
}
