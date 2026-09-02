use super::super::test_support::options;
use super::TranscriptStore;
use crate::llm::ToolCallStreamProgress;
use crate::render::transcript::unified_diff;

/// 构造 write_file 工具结果输出。
///
/// 参数:
/// - `path`: 展示用路径
/// - `old`: 写盘前内容
/// - `new`: 写盘后内容
///
/// 返回:
/// - 与工具输出同构的 JSON 文本
fn write_file_output(path: &str, old: &str, new: &str) -> String {
    let (added, removed) = crate::tools::file_diff::diff_line_counts(old, new);
    serde_json::json!({
        "ok": true,
        "mode": "write",
        "changed_files": [{
            "action": if old.is_empty() { "Added" } else { "Edited" },
            "path": path,
            "added": added,
            "removed": removed,
            "diff": crate::tools::file_diff::unified_diff(path, old, new)
        }]
    })
    .to_string()
}

/// 【TUI】【写入定稿】工具结果先于渲染到达时，统计与 diff 正文必须保留。
///
/// 复现真实时序：参数流结束后文件立即写盘，事件进 TUI 时磁盘上已是新内容；
/// 事件层按参数重建预览会得到空差异并报 +0 -0。
#[test]
fn tool_result_restores_diff_when_file_already_written() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("MOJITO-14143.xml");
    let old = "<root>old</root>\n";
    let new = "<root>new</root>\n";
    std::fs::write(&path, old).unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "content": new
    })
    .to_string();
    let mut store = TranscriptStore::new(100);

    // 1. 参数流阶段：流式统计随分片跳动
    store.push_tool_call_progress(&ToolCallStreamProgress {
        index: 0,
        name: Some("write_file".to_string()),
        arguments_chars: arguments.len(),
        arguments_bytes: arguments.len(),
        arguments_preview: arguments.clone(),
    });
    // 2. 工具调用定稿并写盘，随后才由 TUI 消费事件
    store.push_tool_call("write_file".to_string(), arguments);
    std::fs::write(&path, new).unwrap();
    // 3. 工具结果携带写盘前的 diff 与统计
    store.push_tool_result(
        "write_file".to_string(),
        true,
        write_file_output("MOJITO-14143.xml", old, new),
    );

    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    let first = plain.lines().next().unwrap_or_default();
    assert!(first.contains("Wrote"), "{first}");
    assert!(first.contains("MOJITO-14143.xml"), "{first}");
    assert!(first.contains("+1"), "徽标不应归零: {first}");
    assert!(first.contains("-1"), "徽标不应归零: {first}");
    assert!(plain.contains("old"), "diff 正文不应丢失: {plain}");
    assert!(plain.contains("new"), "diff 正文不应丢失: {plain}");
}

/// 【TUI】【写入定稿】结果报告缺失时保留流式阶段冻结的快照。
#[test]
fn missing_result_report_keeps_frozen_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("keep.txt");
    let old = "alpha\n";
    let new = "beta\n";
    std::fs::write(&path, old).unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "content": new
    })
    .to_string();
    let mut store = TranscriptStore::new(100);

    store.push_tool_call("write_file".to_string(), arguments);
    std::fs::write(&path, new).unwrap();
    store.push_tool_result("write_file".to_string(), true, "{}".to_string());

    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    assert!(plain.contains("alpha"), "{plain}");
    assert!(plain.contains("beta"), "{plain}");
}

/// 【TUI】【写入定稿】失败结果保留预览，不回退成错误状态行。
#[test]
fn failed_write_keeps_preview_body() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("fail.txt");
    std::fs::write(&path, "alpha\n").unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "content": "beta\n"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);

    store.push_tool_call("write_file".to_string(), arguments);
    store.push_tool_result(
        "write_file".to_string(),
        false,
        "tool error: write failed".to_string(),
    );

    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    assert!(plain.contains("alpha"), "{plain}");
    assert!(plain.contains("beta"), "{plain}");
    assert!(plain.contains("err"), "{plain}");
}

/// 【TUI】【结果 diff 解析】unified diff 行号与增删标记被正确恢复。
#[test]
fn parsed_unified_diff_keeps_line_numbers_and_markers() {
    let diff =
        crate::tools::file_diff::unified_diff("a.txt", "one\ntwo\nthree\n", "one\nTWO\nthree\n");
    let patch = unified_diff::parse_unified_diff(&diff).unwrap();

    assert_eq!(patch.path, "a.txt");
    let delete = patch
        .lines
        .iter()
        .find(|line| line.kind == unified_diff::UnifiedLineKind::Delete)
        .unwrap();
    let add = patch
        .lines
        .iter()
        .find(|line| line.kind == unified_diff::UnifiedLineKind::Add)
        .unwrap();
    // hunk 头的旧起点是本 hunk 第一行（上下文 one）的行号 1，
    // 删除 two 排在 hunk 第二行
    assert_eq!(delete.old_line, Some(2));
    assert_eq!(add.new_line, Some(2));
    assert_eq!(delete.text, "two");
    assert_eq!(add.text, "TWO");
    assert!(patch
        .lines
        .iter()
        .any(|line| line.kind == unified_diff::UnifiedLineKind::Context
            && line.old_line == Some(1)
            && line.new_line == Some(1)));
}
