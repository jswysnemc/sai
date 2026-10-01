use super::hover::HOVER_BG;
use super::runtime_tests::{mouse, runtime};
use crossterm::event::Event;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

/// 【全屏测试】【悬停辅助】返回上一帧指定屏幕行的原始 ANSI 文本。
/// @param runtime 为运行期；row 为屏幕行
/// @returns 该行 ANSI 文本
fn painted_row(runtime: &crate::cli::repl_runtime::ReplRuntime, row: u16) -> String {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()[usize::from(row)]
    .clone()
}

/// 鼠标停在折叠段上整段铺悬停底色，移开后恢复；只在悬停段落变化时重绘。
#[test]
fn hovering_an_expandable_paragraph_highlights_it() {
    let mut runtime = runtime(2);
    runtime.enter_fullscreen().unwrap();
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(
            KeyCode::Home,
            KeyModifiers::CONTROL,
        )))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let span = session.state.document.paragraphs[0].clone();
    let layout = session.state.layout.unwrap();
    let scroll = session.state.scroll;
    let title_row = layout.body_top + (span.start - scroll) as u16;
    let last_row = layout.body_top + (span.end - 1 - scroll) as u16;
    assert!(!painted_row(&runtime, title_row).contains(HOVER_BG));
    // 1. 指向段落标题：整段铺底色
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, title_row))
        .unwrap();
    assert!(painted_row(&runtime, title_row).contains(HOVER_BG));
    assert!(
        painted_row(&runtime, last_row).contains(HOVER_BG),
        "整段都应高亮"
    );
    // 2. 在同一段内移动不改变悬停段落
    let before = runtime.fullscreen.as_ref().unwrap().state.previous.clone();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 6, title_row))
        .unwrap();
    assert_eq!(runtime.fullscreen.as_ref().unwrap().state.previous, before);
    // 3. 移到段落之外：底色消失
    let outside = (0..layout.body_height)
        .map(|row| layout.body_top + row)
        .find(|row| {
            let document_row = scroll + usize::from(row - layout.body_top);
            runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .document
                .paragraph_at(document_row)
                .is_none()
        })
        .expect("正文中应有不可展开的行");
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, outside))
        .unwrap();
    assert!(!painted_row(&runtime, title_row).contains(HOVER_BG));
}

/// 离开底部后按钮水平居中贴在输入框正上方，浮动标题不再重复显示新输出提示。
#[test]
fn bottom_button_floats_centered_above_the_composer() {
    let mut runtime = runtime(4);
    runtime.enter_fullscreen().unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 10, 5))
        .unwrap();
    runtime
        .record_runner_event(&crate::runner::RunnerEvent::Agent(
            crate::agent::AgentEvent::Chunk(crate::llm::ChatStreamChunk {
                kind: crate::llm::ChatStreamKind::Content,
                text: "fresh output\n\nmore\n".into(),
            }),
        ))
        .unwrap();
    runtime.paint_fullscreen().unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    assert!(session.state.unseen);
    // 1. 按钮在正文最后一行，紧贴输入框，水平居中
    let (start, end) = session.bottom_button.expect("button while scrolled up");
    let width = usize::from(layout.content_width);
    let center = (usize::from(start) + usize::from(end)) / 2;
    assert!(
        center.abs_diff(width / 2) <= 1,
        "按钮应水平居中: {start}..{end} / {width}"
    );
    let last_row = layout.body_top + layout.body_height - 1;
    assert_eq!(last_row + 1, layout.composer_top, "按钮行紧贴输入框");
    let button_row =
        crate::render::activity_animation::strip_ansi_for_test(&painted_row(&runtime, last_row));
    assert!(
        button_row.contains("有新输出") || button_row.contains("New output"),
        "{button_row}"
    );
    // 2. 浮动标题右上角不再出现新输出提示
    let header = crate::render::activity_animation::strip_ansi_for_test(&painted_row(&runtime, 0));
    assert!(
        !header.contains("有新输出") && !header.contains("New output"),
        "{header}"
    );
}
