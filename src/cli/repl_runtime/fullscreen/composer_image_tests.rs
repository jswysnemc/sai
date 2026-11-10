//! 输入区图片命中与正文浮窗复用回归。

use super::{click, fixture, text};
use crate::cli::repl_runtime::fullscreen::runtime_tests::{chrome, mouse};
use crate::cli::repl_runtime::ReplRuntime;
use crate::render::terminal_image::test_override;
use crossterm::event::{Event, MouseButton, MouseEventKind};

/// 【输入图片测试】【来源定位】从最后绘制签名查找指定附件，不依赖固定底栏高度。
/// 参数: runtime 为运行期，source 为来源；返回: 可见缩略图内部坐标。
fn position(runtime: &ReplRuntime, source: &str) -> (u16, u16) {
    let signature = runtime.last_composer_signature.as_ref().unwrap();
    let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
    (layout.composer_top..layout.rows)
        .find_map(|row| {
            (0..layout.cols).find_map(|column| {
                (signature
                    .image_source_at(column, row)
                    .map(|source| &**source)
                    == Some(source))
                .then_some((column + 1, row + 1))
            })
        })
        .expect("visible composer image")
}

/// 【输入图片测试】【草稿设置】更新草稿和图片来源，覆盖空闲与流式共享绘制入口。
/// 参数: runtime 为运行期，sources 为附件来源；返回: 无。
fn set_images(runtime: &mut ReplRuntime, sources: Vec<String>) {
    runtime
        .update_composer_content(&chrome(), "kept draft", 4, false, Vec::new(), 0, sources)
        .unwrap();
    runtime.draw_composer().unwrap();
}

/// 【输入图片测试】【复用浮窗】文件和数据图片均进入正文查看器，关闭后保留草稿与附件。
/// 参数: 无；返回: 无。
#[test]
fn composer_images_open_shared_viewer_and_restore_draft() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let (_directory, mut runtime, path) = fixture(true);
    let data = format!(
        "data:image/png;base64,{}",
        STANDARD.encode(std::fs::read(&path).unwrap())
    );
    for streaming in [false, true] {
        runtime.stream_active = streaming;
        set_images(&mut runtime, vec![path.clone(), data.clone()]);
        let before = runtime.last_composer_signature.clone();
        for source in [&path, &data] {
            let (column, row) = position(&runtime, source);
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
            assert!(text(&runtime).contains("Image ·"));
            assert!(!text(&runtime).contains("data:image"));
            click(&mut runtime, 0, 0);
            assert!(!runtime.image_viewer_active());
            assert!(runtime.last_composer_signature == before);
            assert!(runtime
                .composer_selection
                .take_cursor("kept draft")
                .is_none());
        }
    }
    test_override::set(None, None, None);
}

/// 【输入图片测试】【拖动与失效】拖动、跨缩略图、缩放和附件移除都不能打开旧图片。
/// 参数: 无；返回: 无。
#[test]
fn composer_image_click_cancels_on_drag_resize_or_removal() {
    let (_directory, mut runtime, path) = fixture(true);
    set_images(&mut runtime, vec![path.clone(), path.clone()]);
    let (column, row) = position(&runtime, &path);
    for events in [
        vec![
            mouse(MouseEventKind::Down(MouseButton::Left), column, row),
            mouse(MouseEventKind::Drag(MouseButton::Left), column + 1, row),
            mouse(MouseEventKind::Up(MouseButton::Left), column, row),
        ],
        vec![
            mouse(MouseEventKind::Down(MouseButton::Left), column, row),
            mouse(MouseEventKind::Up(MouseButton::Left), column + 7, row),
        ],
        vec![
            mouse(MouseEventKind::Down(MouseButton::Left), column, row),
            Event::Resize(60, 20),
            mouse(MouseEventKind::Up(MouseButton::Left), column, row),
        ],
    ] {
        for event in events {
            runtime.handle_fullscreen_event(&event).unwrap();
        }
        assert!(!runtime.image_viewer_active());
    }
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), column, row))
        .unwrap();
    set_images(&mut runtime, Vec::new());
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), column, row))
        .unwrap();
    assert!(!runtime.image_viewer_active());
    assert!(runtime
        .composer_selection
        .take_cursor("kept draft")
        .is_none());
    test_override::set(None, None, None);
}
