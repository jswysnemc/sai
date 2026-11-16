//! 工具详情查看保护与恢复自动收缩回归测试。

use super::runtime_tests::{chrome, mouse};
use crate::cli::repl_runtime::ReplRuntime;
use crate::config::TuiMode;
use crate::render::activity_animation::strip_ansi_for_test as plain;
use crate::render::transcript::TranscriptRenderOptions;
use crate::render::{tui_mode, ReasoningDisplayMode, ToolCallDisplayMode};
use crossterm::event::{MouseButton, MouseEventKind};

fn full_options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    }
}

fn click(runtime: &mut ReplRuntime, column: u16, row: u16) {
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), column, row))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), column, row))
        .unwrap();
}

fn push_bash(runtime: &mut ReplRuntime, command: &str, output: &str) {
    runtime
        .transcript
        .push_tool_call("bash".into(), format!(r#"{{"command":"{command}"}}"#));
    runtime
        .transcript
        .push_tool_result("bash".into(), true, output.into());
}

/// 【终端】【防永久锁死】停在底部时短暂悬停再移开鼠标，不得永久禁用后续工具详情自动收缩。
#[test]
fn transient_hover_without_pause_preserves_auto_collapse() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut runtime = ReplRuntime::new(200, full_options());
    push_bash(&mut runtime, "echo first", "FIRST-DETAIL-LINE");
    runtime.enter_fullscreen().unwrap();

    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let doc_row = session
        .state
        .document
        .lines
        .iter()
        .position(|line| plain(line.as_str()).contains("FIRST-DETAIL-LINE"))
        .unwrap();
    let screen_row = layout.body_top + (doc_row - session.state.scroll) as u16;

    // 1. 鼠标经过首个工具且期间有一次短刷新，随后移回输入框与侧栏区
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 6, screen_row))
        .unwrap();
    push_bash(&mut runtime, "echo brief", "SECOND-SHORT");
    runtime.paint_fullscreen().unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 6, layout.composer_top))
        .unwrap();
    assert!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .state
            .pinned_cells
            .is_empty(),
        "未脱离底部的瞬时悬停不应写入持久固定锁"
    );

    // 2. 再推入超过 10 行的后续工具结果，旧工具详情必须恢复正常自动收缩
    let large_out = (0..12)
        .map(|i| format!("LARGE-LINE-{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    push_bash(&mut runtime, "echo large", &large_out);
    runtime.paint_fullscreen().unwrap();

    let session = runtime.fullscreen.as_ref().unwrap();
    let screen = session
        .state
        .document
        .lines
        .iter()
        .map(|line| plain(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !screen.contains("FIRST-DETAIL-LINE"),
        "鼠标移开后超过十行窗口的旧工具应自动收缩:\n{screen}"
    );
    assert!(screen.contains("LARGE-LINE-11"));
    tui_mode::set_override(None);
}

/// 【终端】【悬停后恢复】悬停暂停期间保持展开，点击回到底部后立即恢复自动收缩。
#[test]
fn clicking_bottom_after_hover_pause_restores_auto_collapse() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut runtime = ReplRuntime::new(200, full_options());
    push_bash(&mut runtime, "echo step-0", "INSPECT-HOLD-LINE");
    runtime.enter_fullscreen().unwrap();

    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let row = session
        .state
        .document
        .lines
        .iter()
        .position(|line| plain(line.as_str()).contains("INSPECT-HOLD-LINE"))
        .unwrap();
    let screen_row = layout.body_top + (row - session.state.scroll) as u16;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 6, screen_row))
        .unwrap();

    for idx in 0..5 {
        let out = (0..8)
            .map(|line| format!("BATCH-{idx}-ROW-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        push_bash(&mut runtime, &format!("echo {idx}"), &out);
    }
    runtime.paint_fullscreen().unwrap();

    // 1. 仅工具链超出内部窗口时，执行链保持暂停并显示点击恢复跟随入口
    let session = runtime.fullscreen.as_ref().unwrap();
    assert!(!session.state.activity.all_following());
    let footer_doc_row = session.state.document.activities[0]
        .footer
        .expect("超出预算的执行链应显示恢复跟随页脚");
    let footer_screen_row = layout.body_top + (footer_doc_row - session.state.scroll) as u16;
    click(&mut runtime, 6, footer_screen_row);

    let session = runtime.fullscreen.as_ref().unwrap();
    assert!(session.state.activity.all_following());
    assert!(session.state.pinned_cells.is_empty());
    let screen = session
        .state
        .document
        .lines
        .iter()
        .map(|line| plain(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !screen.contains("INSPECT-HOLD-LINE"),
        "点击恢复跟随后原本被悬停保活的旧工具应恢复自动收缩:\n{screen}"
    );
    tui_mode::set_override(None);
}

/// 【终端】【手动展开恢复】手动展开工具阅读期间不被大量新工具收起，回到或滚到底部后恢复自动收缩。
#[test]
fn manual_expand_protects_until_bottom_then_restores_collapse() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut runtime = ReplRuntime::new(200, full_options());
    for idx in 0..3 {
        push_bash(
            &mut runtime,
            &format!("echo task-{idx}"),
            &format!("TARGET-{idx}-DETAIL\nSUBLINE-{idx}"),
        );
    }
    runtime.enter_fullscreen().unwrap();

    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let row = session
        .state
        .document
        .lines
        .iter()
        .position(|line| plain(line.as_str()).contains("TARGET-0-DETAIL"))
        .unwrap();
    let screen_row = layout.body_top + (row - session.state.scroll) as u16;
    // 1. 先收起再手动展开首条，模拟主动固定查看工具详情
    click(&mut runtime, 6, screen_row);
    let ctrl = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .paragraphs
        .iter()
        .find(|span| {
            span.key.cell == 0
                && !matches!(
                    span.key.part,
                    crate::render::transcript::ParagraphPart::ActivityGroup
                        | crate::render::transcript::ParagraphPart::OlderActivity
                )
        })
        .unwrap()
        .control;
    let expand_row =
        layout.body_top + (ctrl - runtime.fullscreen.as_ref().unwrap().state.scroll) as u16;
    click(&mut runtime, 6, expand_row);

    for idx in 3..17 {
        push_bash(
            &mut runtime,
            &format!("echo later-{idx}"),
            &format!("LATER-LINE-{idx}"),
        );
    }
    runtime.paint_fullscreen().unwrap();
    assert!(runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .lines
        .iter()
        .any(|line| plain(line.as_str()).contains("TARGET-0-DETAIL")));

    // 2. 点击回到底部恢复自动跟随，旧步骤重新收缩
    let session = runtime.fullscreen.as_ref().unwrap();
    let (start, end) = session.bottom_button.expect("应显示有新输出按钮");
    click(
        &mut runtime,
        (start + end) / 2,
        layout.body_top + layout.body_height - 1,
    );

    let session = runtime.fullscreen.as_ref().unwrap();
    let screen = session
        .state
        .document
        .lines
        .iter()
        .map(|line| plain(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(session.state.follow);
    assert!(session.state.pinned_cells.is_empty());
    assert!(
        !screen.contains("TARGET-0-DETAIL"),
        "回到底部后超过十行的已读详情应恢复收缩:\n{screen}"
    );
    tui_mode::set_override(None);
}

/// 【终端】【底部下滚】一屏内未产生滚动差时，主动向下滚轮同样释放锁定并恢复自动收缩。
#[test]
fn scrolling_down_at_max_releases_pinned_tools() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut runtime = ReplRuntime::new(200, full_options());
    push_bash(&mut runtime, "echo one", "ONLY-ONE-BODY");
    runtime
        .update_composer(&chrome(), "", 0, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();

    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let row = session
        .state
        .document
        .lines
        .iter()
        .position(|line| plain(line.as_str()).contains("ONLY-ONE-BODY"))
        .unwrap();
    // 1. 一屏能装下全部内容时，先点击收起再点展开，触发锁定但 scroll == max == 0
    click(&mut runtime, 6, layout.body_top + row as u16);
    let ctrl = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .paragraphs
        .iter()
        .find(|span| span.key.cell == 0)
        .unwrap()
        .control;
    click(&mut runtime, 6, layout.body_top + ctrl as u16);
    assert!(!runtime.fullscreen.as_ref().unwrap().state.follow);
    assert!(!runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .pinned_cells
        .is_empty());

    // 2. 底部继续向下滚动滚轮，必须释放查看锁并恢复跟随
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::ScrollDown,
            6,
            layout.body_top + layout.body_height.saturating_sub(1),
        ))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state.follow);
    assert!(state.pinned_cells.is_empty());
    tui_mode::set_override(None);
}
