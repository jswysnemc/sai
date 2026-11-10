use super::image_viewer_layout::ViewerLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use crate::cli::repl_runtime::{viewport::TerminalSize, FullscreenEvent, ReplRuntime};
use crate::render::{ansi_clip::strip_ansi, terminal_image::test_override};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

#[path = "composer_image_tests.rs"]
mod composer;

/// 【图片浮窗测试】【图片样本】创建非透明大图，并通过真实读图卡片进入全屏。
/// 参数: kitty 为协议能力；返回: 临时目录、运行期和原图路径。
fn fixture(kitty: bool) -> (tempfile::TempDir, ReplRuntime, String) {
    test_override::set(Some(kitty), Some(false), Some(false));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.png");
    image::RgbaImage::from_pixel(640, 480, image::Rgba([47, 121, 197, 255]))
        .save(&path)
        .unwrap();
    let path = path.to_string_lossy().into_owned();
    let mut runtime = runtime(0);
    runtime.transcript.push_tool_call(
        "read_file".into(),
        serde_json::json!({"path":path}).to_string(),
    );
    runtime.transcript.push_tool_result(
        "read_file".into(),
        true,
        format!("[Image: source: {path}, image/png, 640x480]"),
    );
    runtime
        .update_composer(&chrome(), "kept draft", 10, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    (directory, runtime, path)
}

/// 【图片浮窗测试】【点击】发送完整点击，避免只测试按钮按下。
/// 参数: runtime 为运行期，column/row 为屏幕位置；返回: 无。
fn click(runtime: &mut ReplRuntime, column: u16, row: u16) {
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        assert_eq!(
            runtime
                .handle_fullscreen_event(&mouse(kind, column, row))
                .unwrap(),
            FullscreenEvent::Consumed
        );
    }
}

/// 【图片浮窗测试】【图片位置】使用行侧元数据定位缩略图内部。
/// 参数: runtime 为运行期；返回: 可点击的屏幕列行。
fn image_position(runtime: &ReplRuntime) -> (u16, u16) {
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let layout = state.layout.unwrap();
    state
        .document
        .lines
        .iter()
        .enumerate()
        .skip(state.scroll)
        .take(usize::from(layout.body_height))
        .find_map(|(row, line)| {
            line.image.as_ref().map(|image| {
                (
                    image.column as u16 + 1,
                    layout.body_top + (row - state.scroll) as u16,
                )
            })
        })
        .expect("visible image metadata")
}

/// 【图片浮窗测试】【屏幕文字】读取差异绘制基线中的可见文本。
/// 参数: runtime 为运行期；返回: 当前正文屏幕文本。
fn text(runtime: &ReplRuntime) -> String {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()
        .iter()
        .map(|line| strip_ansi(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【图片浮窗测试】【点击放大与恢复】原图放置大于缩略图，关闭不改变阅读位置或草稿。
/// 参数: 无；返回: 无。
#[test]
fn clicking_thumbnail_enlarges_and_escape_restores_chat() {
    let (_directory, mut runtime, path) = fixture(true);
    let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
    let expanded = runtime.fullscreen.as_ref().unwrap().state.expanded.clone();
    let composer = runtime.last_composer_signature.clone();
    let (column, row) = image_position(&runtime);
    click(&mut runtime, column, row);
    assert_eq!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .image_viewer
            .source
            .as_deref(),
        Some(path.as_str())
    );
    assert!(text(&runtime).contains("sample.png"));
    let protocol = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()
        .join("\n");
    let (width, _) = crate::render::image_reference::dimensions(&protocol);
    assert!(width > 24, "{protocol}");
    assert!(!protocol.contains("sai-image;"));
    for event in [
        Event::Paste("must not insert".into()),
        Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
        mouse(MouseEventKind::ScrollUp, column, row),
    ] {
        assert_eq!(
            runtime.handle_fullscreen_event(&event).unwrap(),
            FullscreenEvent::Consumed
        );
    }
    assert!(runtime.last_composer_signature == composer);
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(!runtime.image_viewer_active());
    assert_eq!(state.scroll, before);
    assert_eq!(state.expanded, expanded);
    assert!(text(&runtime).contains("Viewed image"));
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【尺寸与关闭】缩放后使用新按钮坐标，图片始终位于正文区内。
/// 参数: 无；返回: 无。
#[test]
fn resize_and_close_button_work_with_character_fallback() {
    let (_directory, mut runtime, _) = fixture(false);
    let (column, row) = image_position(&runtime);
    click(&mut runtime, column, row);
    assert!(runtime.image_viewer_active());
    for size in [
        TerminalSize { cols: 42, rows: 18 },
        TerminalSize { cols: 12, rows: 6 },
    ] {
        TerminalSize::with_size(size, || {
            runtime.repaint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let geometry = ViewerLayout::compute(&layout);
            assert!(geometry.top + geometry.height <= usize::from(layout.composer_top));
            for line in session.state.previous.as_ref().unwrap() {
                assert!(
                    unicode_width::UnicodeWidthStr::width(strip_ansi(line).as_str())
                        <= usize::from(size.cols)
                );
            }
        });
    }
    runtime.repaint_fullscreen().unwrap();
    let geometry =
        ViewerLayout::compute(&runtime.fullscreen.as_ref().unwrap().state.layout.unwrap());
    let column = (geometry.left + geometry.width - 3) as u16;
    let row = geometry.top as u16;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, column, row))
        .unwrap();
    assert!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .image_viewer
            .close_hovered
    );
    click(&mut runtime, column, row);
    assert!(!runtime.image_viewer_active());
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【拖选隔离】图片附近拖选仍是正文选区，不意外打开浮窗。
/// 参数: 无；返回: 无。
#[test]
fn dragging_over_an_image_is_not_a_click() {
    let (_directory, mut runtime, _) = fixture(true);
    let (column, row) = image_position(&runtime);
    for event in [
        mouse(MouseEventKind::Down(MouseButton::Left), column, row),
        mouse(MouseEventKind::Drag(MouseButton::Left), column + 2, row),
        mouse(MouseEventKind::Up(MouseButton::Left), column + 2, row),
    ] {
        runtime.handle_fullscreen_event(&event).unwrap();
    }
    assert!(!runtime.image_viewer_active());
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【失效来源】打开后文件删除，浮窗显示错误且仍能关闭。
/// 参数: 无；返回: 无。
#[test]
fn missing_source_does_not_trap_the_viewer() {
    let (_directory, mut runtime, path) = fixture(true);
    let (column, row) = image_position(&runtime);
    std::fs::remove_file(path).unwrap();
    click(&mut runtime, column, row);
    assert!(runtime.image_viewer_active());
    assert!(
        text(&runtime).contains("Image preview unavailable.")
            || text(&runtime).contains("图片预览不可用。")
    );
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        .unwrap();
    assert!(!runtime.image_viewer_active());
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【用户数据图片】展开后的数据 URL 附件保留来源，点击占位行也能放大。
/// 参数: 无；返回: 无。
#[test]
fn user_data_image_opens_from_a_reserved_row() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let (_directory, mut runtime, path) = fixture(true);
    let source = format!(
        "data:image/png;base64,{}",
        STANDARD.encode(std::fs::read(path).unwrap())
    );
    runtime.transcript.push_user_input(
        crate::render::transcript::TranscriptMode::Yolo,
        crate::render::input_atom::InputEcho::with_images(
            "attachment".into(),
            std::slice::from_ref(&source),
        ),
    );
    runtime.paint_fullscreen().unwrap();
    let key = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .paragraphs
        .last()
        .unwrap()
        .key;
    runtime
        .fullscreen
        .as_mut()
        .unwrap()
        .state
        .expanded
        .insert(key);
    runtime.paint_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let layout = state.layout.unwrap();
    let (row, image) = state
        .document
        .lines
        .iter()
        .enumerate()
        .rev()
        .find_map(|(row, line)| {
            line.image
                .as_ref()
                .filter(|image| &*image.source == source)
                .map(|image| (row, image))
        })
        .expect("user image metadata in the last reserved row");
    let column = image.column as u16 + 1;
    let row = layout.body_top + (row - state.scroll) as u16;
    click(&mut runtime, column, row);
    assert_eq!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .image_viewer
            .source
            .as_deref(),
        Some(source.as_str())
    );
    assert!(!text(&runtime).contains("data:image"));
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【图片外关闭】字符与图形预览均按实际图片范围关闭，留白和输入区也可返回。
/// 参数: 无；返回: 无。
#[test]
fn outside_click_closes_without_clicking_through() {
    for kitty in [true, false] {
        let (_directory, mut runtime, _) = fixture(kitty);
        for target in 0..3 {
            let (column, row) = image_position(&runtime);
            click(&mut runtime, column, row);
            let area = runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .image_viewer
                .image_area
                .unwrap();
            for (column, row) in [
                (area.left, area.top),
                (area.left + area.width - 1, area.top + area.height - 1),
            ] {
                click(&mut runtime, column as u16, row as u16);
                assert!(runtime.image_viewer_active());
            }
            let composer = runtime.last_composer_signature.clone();
            let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
            let (column, row) = match target {
                0 => (area.left - 1, area.top),
                1 => (0, 0),
                _ => (3, usize::from(layout.composer_top) + 1),
            };
            click(&mut runtime, column as u16, row as u16);
            assert!(!runtime.image_viewer_active());
            assert!(runtime.last_composer_signature == composer);
            assert!(text(&runtime).contains("Viewed image"));
            assert!(runtime
                .composer_selection
                .take_cursor("kept draft")
                .is_none());
        }
    }
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【关闭手势】拖动、单独释放与跨越图片边界都不能触发关闭。
/// 参数: 无；返回: 无。
#[test]
fn dismissal_requires_a_complete_click_outside_the_image() {
    let (_directory, mut runtime, _) = fixture(true);
    let (column, row) = image_position(&runtime);
    click(&mut runtime, column, row);
    let area = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .image_viewer
        .image_area
        .unwrap();
    for events in [
        vec![mouse(MouseEventKind::Up(MouseButton::Left), 0, 0)],
        vec![
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                area.left as u16,
                area.top as u16,
            ),
            mouse(MouseEventKind::Up(MouseButton::Left), 0, 0),
        ],
        vec![
            mouse(MouseEventKind::Down(MouseButton::Left), 0, 0),
            mouse(MouseEventKind::Drag(MouseButton::Left), 1, 0),
            mouse(MouseEventKind::Up(MouseButton::Left), 0, 0),
        ],
        vec![
            mouse(MouseEventKind::Down(MouseButton::Left), 0, 0),
            Event::Resize(60, 20),
            mouse(MouseEventKind::Up(MouseButton::Left), 0, 0),
        ],
    ] {
        for event in events {
            runtime.handle_fullscreen_event(&event).unwrap();
        }
        assert!(runtime.image_viewer_active());
    }
    click(&mut runtime, 0, 0);
    assert!(!runtime.image_viewer_active());
    test_override::set(None, None, None);
}
