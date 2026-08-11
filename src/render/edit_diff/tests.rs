use super::renderer::render_for_test;
use serde_json::json;

#[test]
fn repl_diff_keeps_symmetric_background_insets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "old\n").unwrap();
    let args = json!({
        "path": path.display().to_string(),
        "old_string": "old",
        "new_string": "new"
    })
    .to_string();

    let output = render_for_test(&args).unwrap();

    assert!(output.contains("\x1b[K"));
    assert!(output.contains("\x1b[2D\x1b[3X"));
}

/// 【终端】【Diff 换行】验证 CLI diff 长行也走显式折行。
///
/// TUI 侧折行时会为续行恢复背景与正文缩进，CLI 侧此前不折行，
/// 交给终端硬换行：续行落在第 0 列且不带背景色，增删行的矩形色块
/// 因此在每个续行行首缺一个口子。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn cli_diff_wraps_long_lines_with_background_continuation() {
    use crate::render::render_width::with_render_width;

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("long.txt");
    std::fs::write(&path, "short\n").unwrap();
    let args = json!({
        "path": path.display().to_string(),
        "old_string": "short",
        "new_string": "y".repeat(200)
    })
    .to_string();

    let width = 60usize;
    let output = with_render_width(width, || render_for_test(&args).unwrap());

    for line in output.lines() {
        let plain = crate::render::activity_animation::strip_ansi_for_test(line);
        let visible: usize = plain
            .chars()
            .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum();
        assert!(visible <= width, "diff 行宽 {visible} 超出终端: {plain:?}");
    }

    // 续行必须先恢复背景再补缩进，否则缩进列落在终端默认背景上，
    // 增删行的矩形色块在每个续行行首缺一个口子
    let continuations = output
        .lines()
        .filter(|line| {
            let plain = crate::render::activity_animation::strip_ansi_for_test(line);
            plain.contains('y') && !plain.contains('+')
        })
        .collect::<Vec<_>>();
    assert!(!continuations.is_empty(), "样例必须触发折行");
    for line in continuations {
        let background_at = line.find("\x1b[48;5;").expect("续行必须带 diff 背景色");
        let content_at = line.find('y').expect("续行必须含正文");
        assert!(
            background_at < content_at,
            "背景色必须在正文与缩进之前: {line:?}"
        );
    }
}
