use super::{options, strip_ansi, TranscriptStore};

/// 【终端】【Diff 换行】验证长行折行后续行缩进到 diff 正文列。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn diff_long_lines_wrap_with_body_column_indent() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("long.md");
    std::fs::write(&path, "short\n").unwrap();
    // 用不会出现在临时路径里的字符，否则标题行会被误判为长行的一部分
    let long = "\u{4e2d}".repeat(160);
    let args = serde_json::json!({
        "path": path.display().to_string(),
        "old_string": "short",
        "new_string": long
    })
    .to_string();

    let mut store = TranscriptStore::new(100);
    store.push_tool_call("str_replace".to_string(), args);
    let lines = store.display_window(60, &options(), 40, usize::MAX);
    let plain = lines
        .lines
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .filter(|line| line.contains('\u{4e2d}'))
        .collect::<Vec<_>>();

    assert!(plain.len() > 1, "长行应被折成多行: {plain:?}");
    // 首行带行号与标记，其余是续行；续行必须缩进到正文列而非顶在最左侧
    let continuations = plain
        .iter()
        .filter(|line| !line.contains('+'))
        .collect::<Vec<_>>();
    assert!(!continuations.is_empty(), "应存在续行: {plain:?}");
    for line in continuations {
        // 按字符数而非字节数统计，宽字符下两者不等
        let indent = line.chars().take_while(|ch| *ch == ' ').count();
        assert!(
            indent >= 7,
            "续行应缩进到 diff 正文列，实际缩进 {indent}: {line:?}"
        );
    }
}
