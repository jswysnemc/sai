use super::hover_color::hover_default_fg;
use super::runtime_tests::mouse;
use super::state::ActivityNavigation;
use crate::cli::repl_runtime::ReplRuntime;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::ansi_clip::strip_ansi;
use crate::render::transcript::{ParagraphKey, TranscriptRenderOptions};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crossterm::event::{MouseButton, MouseEventKind};

/// 【终端】【块交互夹具】创建使用完整输出模式的全屏运行时。
/// 参数: 无；返回: 尚未进入全屏的运行时。
fn runtime() -> ReplRuntime {
    ReplRuntime::new(
        1000,
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        },
    )
}

/// 【终端】【搜索夹具】创建包含内部隐藏提示的搜索结果卡片。
/// 参数: 无；返回: 已进入全屏的运行时。
fn search_runtime() -> ReplRuntime {
    let mut runtime = runtime();
    runtime.transcript.push_tool_call(
        "grep".into(),
        serde_json::json!({"pattern":"matched"}).to_string(),
    );
    runtime.transcript.push_tool_result(
        "grep".into(),
        true,
        serde_json::json!({
            "ok": true,
            "stdout": (0..30).map(|row| format!("sample.rs:{}:matched-{row}\n", row + 1)).collect::<String>()
        }).to_string(),
    );
    runtime.enter_fullscreen().unwrap();
    runtime
}

/// 【终端】【输出夹具】创建可滚动的完整命令输出，并定位到内部开头。
/// 参数: 无；返回: 已进入全屏的运行时。
fn command_runtime() -> ReplRuntime {
    let mut runtime = runtime();
    runtime.transcript.push_shell(
        "print output".into(),
        (0..60).map(|row| format!("output-{row}\n")).collect(),
        Some(0),
    );
    runtime.enter_fullscreen().unwrap();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    let height = usize::from(state.layout.unwrap().body_height);
    state.navigate_activity(ActivityNavigation::Top, height);
    runtime.paint_fullscreen_interaction().unwrap();
    runtime
}

/// 【终端】【提示定位】读取包含 hidden 的可见提示所在屏幕行。
/// 参数: `runtime` 为全屏运行时；返回: 屏幕行号。
fn hidden_row(runtime: &ReplRuntime) -> u16 {
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let row = state
        .document
        .lines
        .iter()
        .position(|line| strip_ansi(line.as_str()).contains("hidden"))
        .expect("夹具必须包含内部隐藏提示");
    state.layout.unwrap().body_top + (row - state.scroll) as u16
}

/// 【终端】【单击夹具】通过真实事件入口在指定正文行单击。
/// 参数: `runtime` 为运行时，`row` 为屏幕行；返回: 无。
fn click(runtime: &mut ReplRuntime, row: u16) {
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        runtime
            .handle_fullscreen_event(&mouse(kind, 10, row))
            .unwrap();
    }
}

/// 【终端】【思考提示回归】双色滚动条已展示位置时，标题不再附带数字行数和双向箭头。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_reasoning_title_has_no_duplicate_position_hint() {
    let mut runtime = runtime();
    runtime.transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..20).map(|row| format!("thought-{row}\n")).collect(),
    });
    runtime.transcript.finalize_live_tail();
    runtime.enter_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let region = &state.document.reasonings[0];
    let title = strip_ansi(state.document.lines[region.start].as_str());
    assert!(!title.contains('↕'), "标题重复展示滚动位置: {title}");
    assert!(!title.contains("/20"), "标题重复展示行数: {title}");
    assert!(state.document.lines[region.start + 1..region.end]
        .iter()
        .any(|line| line.as_str().contains('│')));
}

/// 【终端】【隐藏提示悬停回归】节点正文中的 hidden 行触发前景提亮，不添加区块背景。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_internal_hidden_line_highlights_on_hover() {
    let mut runtime = search_runtime();
    let row = hidden_row(&runtime);
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 10, row))
        .unwrap();
    let painted = &runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()[usize::from(row)];
    assert!(painted.contains(&hover_default_fg()));
    assert!(!painted.contains("\x1b[48;"));
    assert!(strip_ansi(painted).contains("hidden"));
}

/// 【终端】【隐藏提示展开回归】单击正文 hidden 行展开全文，并保留步骤标题的位置。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_internal_hidden_line_expands_full_output() {
    let mut runtime = search_runtime();
    let row = hidden_row(&runtime);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let header = state.document.paragraphs[0].start - state.scroll;
    assert!(!state.expanded.contains(&ParagraphKey::whole(0)));
    click(&mut runtime, row);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state.expanded.contains(&ParagraphKey::whole(0)));
    assert!(state
        .document
        .paragraphs
        .iter()
        .any(|span| span.key.cell == 0 && span.expanded));
    assert!(state
        .document
        .lines
        .iter()
        .any(|line| strip_ansi(line.as_str()).contains("matched-10")));
    let span = state
        .document
        .paragraphs
        .iter()
        .find(|span| span.key.cell == 0 && span.control == span.start)
        .unwrap();
    assert_eq!(span.start - state.scroll, header);
}

/// 【终端】【全文收起回归】从 hidden 提示展开全文后，正文单击同时关闭全文和节点详情。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_full_output_can_collapse_after_hidden_expansion() {
    let mut runtime = search_runtime();
    let hint = hidden_row(&runtime);
    click(&mut runtime, hint);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let row = state
        .document
        .lines
        .iter()
        .position(|line| strip_ansi(line.as_str()).contains("matched-10"))
        .unwrap();
    let row = state.layout.unwrap().body_top + (row - state.scroll) as u16;
    click(&mut runtime, row);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(!state.expanded.contains(&ParagraphKey::whole(0)));
    assert!(state
        .document
        .lines
        .iter()
        .all(|line| !strip_ansi(line.as_str()).contains("matched-")));
    assert!(!state.document.paragraphs[0].expanded);
}

/// 【终端】【正文收起回归】完整输出正文支持单击收起，再次单击标题恢复输出。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_expanded_output_body_can_collapse_and_reopen() {
    let mut runtime = command_runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let row = state
        .document
        .lines
        .iter()
        .position(|line| line.as_str().contains("output-0"))
        .unwrap();
    let screen_row = state.layout.unwrap().body_top + (row - state.scroll) as u16;
    click(&mut runtime, screen_row);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state
        .document
        .lines
        .iter()
        .all(|line| !line.as_str().contains("output-")));
    assert!(!state.document.paragraphs[0].expanded);
    let row = state.layout.unwrap().body_top
        + (state.document.paragraphs[0].control - state.scroll) as u16;
    click(&mut runtime, row);
    assert!(runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .lines
        .iter()
        .any(|line| line.as_str().contains("output-0")));
}

/// 【终端】【拖选保护回归】展开正文仍支持拖动复制，拖选结束不能触发收起。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_dragging_expanded_output_keeps_it_open() {
    let mut runtime = command_runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let row = state
        .document
        .lines
        .iter()
        .position(|line| line.as_str().contains("output-0"))
        .unwrap();
    let row = state.layout.unwrap().body_top + (row - state.scroll) as u16;
    for (kind, col, row) in [
        (MouseEventKind::Down(MouseButton::Left), 6, row),
        (MouseEventKind::Drag(MouseButton::Left), 12, row + 1),
        (MouseEventKind::Up(MouseButton::Left), 12, row + 1),
    ] {
        runtime
            .handle_fullscreen_event(&mouse(kind, col, row))
            .unwrap();
    }
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state.document.paragraphs[0].expanded);
    assert!(state.copied.is_some());
}

/// 【终端】【输出引导线回归】命令、读取和普通工具的可滚动输出都展示双色左侧滚动条。
/// 参数: 无；返回: 无。
#[test]
fn block_interaction_all_scrollable_tool_outputs_have_two_color_gutters() {
    for name in ["run_command", "read_file", "example_tool"] {
        let mut runtime = runtime();
        runtime.transcript.push_tool_call(
            name.into(),
            serde_json::json!({"command":"print output", "path":"sample.rs"}).to_string(),
        );
        let stdout = (0..100)
            .map(|row| format!("output-{row}\n"))
            .collect::<String>();
        let output = if name == "run_command" {
            serde_json::json!({"success":true,"exit_code":0,"stdout":stdout,"stderr":""})
                .to_string()
        } else {
            stdout
        };
        runtime
            .transcript
            .push_tool_result(name.into(), true, output);
        runtime.enter_fullscreen().unwrap();
        let state = &runtime.fullscreen.as_ref().unwrap().state;
        let region = &state.document.activities[0];
        assert!(region.scrollable, "{name} 夹具必须可滚动");
        let lines = &state.document.lines[region.start + 1..region.footer.unwrap()];
        for line in lines
            .iter()
            .filter(|line| line.as_str().contains("output-"))
        {
            assert_eq!(
                strip_ansi(line.as_str()).matches('│').count(),
                1,
                "{name} 输出必须只包含一条引导线: {line:?}"
            );
        }
        let palette = super::sidebar_palette::SidebarPalette::current();
        for color in [palette.accent, palette.muted] {
            let gutter = format!("{}│", super::sidebar_palette::fg(color));
            assert!(
                lines.iter().any(|line| line.as_str().contains(&gutter)),
                "{name} 缺少双色引导线"
            );
        }
    }
}
