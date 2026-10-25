use super::super::{cell::TranscriptMode, test_support::options, user_echo_cell::UserEchoCell};
use super::*;
use crate::config::TuiStyle;
use crate::render::input_atom::{InputAtom, InputAtomKind, InputEcho};
use crate::render::terminal_image::{reserved_image_rows, test_override};
use crate::render::tui_style;

/// 【终端】【紧凑测试】读取渲染文档的纯文本。
/// 参数: `document` 为渲染文档；返回: 无 ANSI 的文本。
fn text(document: &FullscreenDocument) -> String {
    crate::render::activity_animation::strip_ansi_for_test(
        &document
            .lines
            .iter()
            .map(AnsiLine::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// 【终端】【工具去重测试】历史命令展开后只出现一次命令，正文不含连接线。
/// 参数: 无；返回: 无。
#[test]
fn expanded_command_has_one_title_and_no_guides() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call(
        "run_command".into(),
        r#"{"command":"printf unique-command","timeout":10}"#.into(),
    );
    store.push_tool_result(
        "run_command".into(),
        true,
        r#"{"success":true,"exit_code":0,"stdout":"unique-output","stderr":""}"#.into(),
    );
    let document =
        store.render_fullscreen(100, &options(), &HashSet::from([ParagraphKey::whole(0)]));
    let plain = text(&document);
    assert_eq!(plain.matches("printf unique-command").count(), 1, "{plain}");
    assert!(plain.contains("unique-output"));
    assert!(!plain.contains("\"command\""));
    assert!(!plain.contains("timeout"));
    assert!(!plain.contains('│'));
    assert!(plain.contains('└'));
    tui_style::set_override(None);
}

/// 【终端】【参数完整测试】窄窗口裁剪摘要时，展开正文仍保留完整命令。
/// 参数: 无；返回: 无。
#[test]
fn clipped_summary_keeps_complete_command_in_details() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call(
        "run_command".into(),
        r#"{"command":"printf very-long-command-that-does-not-fit"}"#.into(),
    );
    store.push_tool_result(
        "run_command".into(),
        true,
        r#"{"success":true,"exit_code":0,"stdout":"done","stderr":""}"#.into(),
    );
    let document =
        store.render_fullscreen(30, &options(), &HashSet::from([ParagraphKey::whole(0)]));
    let unfolded = text(&document)
        .lines()
        .map(str::trim_start)
        .collect::<String>();
    assert!(unfolded.contains("printf very-long-command-that-does-not-fit"));
    tui_style::set_override(None);
}

/// 【终端】【用户附件测试】两种布局都默认折叠图片，点击展开使用 Kitty 并保持原有用户圆点。
/// 参数: 无；返回: 无。
#[test]
fn user_images_expand_in_both_styles_without_changing_message_prefix() {
    test_override::set(Some(true), Some(false), Some(false));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("user.png");
    image::RgbaImage::from_pixel(160, 160, image::Rgba([231, 31, 177, 255]))
        .save(&path)
        .unwrap();
    for style in [TuiStyle::Classic, TuiStyle::Deepseek] {
        tui_style::set_override(Some(style));
        let mut store = TranscriptStore::new(200);
        let label = "[image 1]".to_string();
        store.push_cell(HistoryCell::UserEcho(UserEchoCell::with_atoms(
            TranscriptMode::Yolo,
            InputEcho {
                text: label.clone(),
                atoms: vec![InputAtom {
                    range: 0..label.len(),
                    label,
                    kind: InputAtomKind::Image,
                    image_source: Some(path.to_string_lossy().into_owned()),
                }],
            },
        )));
        let collapsed = store.render_fullscreen(80, &options(), &HashSet::new());
        assert!(!collapsed
            .lines
            .iter()
            .any(|line| line.as_str().contains("a=p")));
        assert!(text(&collapsed).contains("● [image 1]"));
        assert_eq!(collapsed.paragraphs.len(), 1);
        let key = collapsed.paragraphs[0].key;
        let expanded = store.render_fullscreen(80, &options(), &HashSet::from([key]));
        let (row, rows) = expanded
            .lines
            .iter()
            .enumerate()
            .find_map(|(row, line)| reserved_image_rows(line.as_str()).map(|rows| (row, rows)))
            .unwrap();
        assert!(rows <= 8);
        assert!(expanded.lines.len() >= row + rows);
        assert!(expanded.paragraphs[0].expanded);
        assert!(!store
            .render_fullscreen(80, &options(), &HashSet::new())
            .lines
            .iter()
            .any(|line| line.as_str().contains("a=p")));
    }
    tui_style::set_override(None);
    test_override::set(None, None, None);
}

/// 【终端】【读图测试】图片读取默认展开，手动收起显示摘要，展开保留占位且不重复路径。
/// 参数: 无；返回: 无。
#[test]
fn image_read_starts_open_and_can_fold_to_a_summary() {
    test_override::set(Some(true), Some(false), Some(false));
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("read.png");
    image::RgbaImage::from_pixel(640, 480, image::Rgba([17, 201, 119, 255]))
        .save(&path)
        .unwrap();
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("Inspect the image".into()));
    store.push_tool_call(
        "read_file".into(),
        serde_json::json!({"path":path}).to_string(),
    );
    store.push_tool_result(
        "read_file".into(),
        true,
        format!(
            "[Image: source: {}, image/png, 1 KB, 640x480]",
            path.display()
        ),
    );
    let collapsed = store.render_fullscreen(
        100,
        &options(),
        &HashSet::from([
            ParagraphKey::activity(0, false),
            ParagraphKey::activity(1, false),
        ]),
    );
    assert!(text(&collapsed).contains("Viewed image"));
    assert_eq!(collapsed.lines.len(), 3);
    assert!(!collapsed
        .lines
        .iter()
        .any(|line| reserved_image_rows(line.as_str()).is_some()));
    let expanded = store.render_fullscreen(100, &options(), &HashSet::new());
    let (row, rows) = expanded
        .lines
        .iter()
        .enumerate()
        .find_map(|(row, line)| reserved_image_rows(line.as_str()).map(|rows| (row, rows)))
        .unwrap();
    assert_eq!(expanded.lines.len(), row + rows);
    assert_eq!(text(&expanded).matches("read.png").count(), 1);
    tui_style::set_override(None);
    test_override::set(None, None, None);
}
