use super::*;
use crate::render::{activity_animation::strip_ansi_for_test, ToolCallDisplayMode};

/// 【终端】【结构化载荷测试】普通工具的嵌套参数和结果使用可读字段及列表，不输出 JSON 语法。
/// 参数: 无；返回: 无。
#[test]
fn generic_tool_renders_readable_fields_and_items() {
    let mut view = ToolView::running("custom_tool".into(), r#"{"pattern":"needle","paths":["src/a.rs","src/b.rs"],"options":{"case_sensitive":true,"limit":20}}"#.into());
    view.finish(
        true,
        r#"{"ok":true,"results":[{"path":"src/a.rs","line":12,"text":"found needle"}],"total":1}"#
            .into(),
    );
    let plain = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(plain.contains("Pattern: needle"), "{plain}");
    assert!(plain.contains("Case sensitive: Yes"), "{plain}");
    assert!(plain.contains("Line: 12"), "{plain}");
    assert!(plain.contains("found needle"), "{plain}");
    assert!(!plain.contains("\"pattern\""), "{plain}");
    assert!(!plain.contains("\"results\""), "{plain}");
}

/// 【终端】【结构化载荷测试】JSON 字符串解码为正文，非 JSON 输出中的代码与换行保持原样。
/// 参数: 无；返回: 无。
#[test]
fn payload_text_keeps_literal_content_without_json_string_quotes() {
    let plain = strip_ansi_for_test(&render_result(
        "custom_tool",
        true,
        r#""first\nsecond""#,
        ToolCallDisplayMode::Full,
    ));
    assert!(plain.contains("first"));
    assert!(plain.contains("second"));
    assert!(!plain.contains("\\n"), "{plain}");
    let literal = "code: print(\"{literal}\")\nnext line";
    let rendered = strip_ansi_for_test(&render_result(
        "custom_tool",
        true,
        literal,
        ToolCallDisplayMode::Full,
    ));
    assert!(rendered.contains("print(\"{literal}\")"));
    assert!(rendered.contains("next line"));
}

/// 【终端】【参数预览测试】未闭合的文件内容保留已接收正文，不显示 JSON 键名和转义。
/// 参数: 无；返回: 无。
#[test]
fn partial_file_content_has_a_readable_preview() {
    let preview =
        structured_payload::format_preview(r#"{"path":"fixture.txt","content":"first\nlatest"#);
    assert!(preview.contains("Path: fixture.txt"), "{preview}");
    assert!(preview.contains("Content: first"), "{preview}");
    assert!(preview.contains("latest"), "{preview}");
    assert!(!preview.contains("\"content\""), "{preview}");
    assert_eq!(
        structured_payload::format_preview("{"),
        "Receiving arguments"
    );
}

/// 【终端】【容器语义测试】空容器、布尔值和多行数组使用语义标签及悬挂缩进。
/// 参数: 无；返回: 无。
#[test]
fn structured_scalars_and_arrays_keep_readable_indentation() {
    let value = serde_json::json!({"items": ["first\ncontinued"], "empty_list": [], "empty_object": {}, "enabled": false, "exitCode": 0, "result": null});
    let formatted = structured_payload::format_value(&value);
    assert!(
        formatted.contains("Items:\n  • first\n    continued"),
        "{formatted}"
    );
    for expected in [
        "Empty list: No items",
        "Empty object: No details",
        "Enabled: No",
        "Exit code: 0",
        "Result: None",
    ] {
        assert!(formatted.contains(expected), "{formatted}");
    }
}
