use super::styled_line::wrap_styled_line;
use super::*;
use crate::agent::AgentMode;
use crate::cli::repl_input_render::repl_prompt_rows_for_cols;
use crate::cli::repl_runtime::viewport::TerminalSize;

/// 【终端】【绘制回归】构造用于检查局部重绘的输入框。
/// 参数: 无
/// 返回: 带固定正文和活动面板的输入框
fn frame() -> ComposerFrame {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "test-model".into(),
        thinking: "auto".into(),
        directory: "/workspace".into(),
        cache_hit_ratio: None,
        status_plugin: None,
    };
    let mut frame = ComposerFrame::new(chrome, "draft".into(), 5, false, Vec::new(), 0);
    frame.set_panel_lines(vec!["Working 1s".into()]);
    frame
}

/// 【终端】【技能补全】候选超出可用高度时，仍显示当前选中的技能
/// 参数: 无；返回: 无，后续技能无法进入可见区域时断言失败
#[test]
fn skill_completion_scrolls_to_selected_item() {
    let mut frame = frame();
    frame.set_panel_lines(Vec::new());
    frame.input = "#".into();
    frame.cursor = 1;
    frame.set_mention_candidates(
        (0..6)
            .map(|index| MentionSuggestion {
                insert: format!("#skill-{index:02}"),
                label: format!("#skill-{index:02}"),
                description: "description".into(),
                continue_filter: false,
            })
            .collect(),
    );
    frame.slash_selection = 5;
    let viewport = InlineViewport::fixed(TerminalSize { cols: 80, rows: 24 }, 17, 7);
    let (_, signature) = frame.draw_lines(&mut Vec::new(), &viewport, None).unwrap();
    assert!(
        signature
            .lines
            .iter()
            .any(|line| line.contains("#skill-05") && line.contains('→')),
        "选中第六项后仍然只显示前五项: {:?}",
        signature.lines,
    );
}

/// 【终端】【技能补全】长列表在窄窗口、短窗口及多行输入下始终显示选中项
/// 参数: 无；返回: 无，选中项不可见或面板无限增高时断言失败
#[test]
fn skill_completion_keeps_selection_visible_after_resize() {
    let mut frame = frame();
    let skills = (0..40)
        .map(|index| (format!("skill-{index:02}"), "description".into()))
        .collect::<Vec<_>>();
    let mut completion = crate::cli::repl_mentions::completion::MentionCompletion::default();
    frame.set_mention_candidates(completion.query("#", 1, &skills));
    for input in ["#", "intro\n#"] {
        frame.input = input.into();
        frame.cursor = input.chars().count();
        for (cols, rows) in [(80, 24), (40, 12), (20, 7), (20, 4)] {
            for selected in 0..skills.len() {
                frame.slash_selection = selected;
                let viewport = InlineViewport::fixed(
                    TerminalSize { cols, rows },
                    0,
                    frame.height(usize::from(cols)).min(rows),
                );
                let (_, signature) = frame.draw_lines(&mut Vec::new(), &viewport, None).unwrap();
                let label = format!("#skill-{selected:02}");
                assert!(
                    signature
                        .lines
                        .iter()
                        .any(|line| line.contains(&label) && line.contains('→')),
                    "{cols}x{rows} 下未显示选中项 {label}: {:?}",
                    signature.lines,
                );
                let cursor_row = usize::from(signature.cursor_row - signature.top);
                assert!(signature.lines[cursor_row].contains('#'));
                assert!(
                    signature
                        .lines
                        .iter()
                        .filter(|line| line.contains("#skill-"))
                        .count()
                        <= 8
                );
            }
        }
    }
}

/// 【终端】【样式折行】技能标签跨行后必须保留背景色与前景色。
/// 参数: 无
/// 返回: 无，样式丢失时断言失败
#[test]
fn regression_wrapped_skill_keeps_its_style() {
    let style = crate::render::input_atom::InputAtomKind::Skill.style();
    let lines = wrap_styled_line(&format!("abcd{style}[#jira-bug-routing]\x1b[0m rest"), 16);
    assert!(lines.len() > 1);
    assert!(
        lines[1].starts_with(style),
        "续行丢失技能样式: {:?}",
        lines[1]
    );
    assert!(lines[0].ends_with("\x1b[0m"), "上一行样式需要收束");
}

/// 【终端】【输入布局】绘制行数应覆盖满行后的光标占位行。
/// 参数: 无
/// 返回: 无，测量与绘制不一致时断言失败
#[test]
fn regression_wrapped_input_matches_measured_rows() {
    for text in ["abcd", "ab中文", "a\tb", "abc\t中"] {
        let lines = wrap_styled_line(text, 4);
        let measured = repl_prompt_rows_for_cols("", &[text.into()], 4);
        assert_eq!(lines.len(), usize::from(measured), "{text:?}");
    }
}

/// 【终端】【局部重绘】活动状态变化不得清空或重写没有变化的输入正文。
/// 参数: 无
/// 返回: 无，重复绘制输入正文时断言失败
#[test]
fn regression_activity_update_does_not_repaint_input() {
    let mut frame = frame();
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 60, rows: 24 }, frame.height(60), 5);
    let (_, signature) = frame.draw_lines(&mut Vec::new(), &viewport, None).unwrap();
    frame.set_panel_lines(vec!["Working 2s".into()]);
    let mut output = Vec::new();
    frame
        .draw_lines(&mut output, &viewport, Some(&signature))
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Working 2s"));
    assert!(
        !output.contains("draft"),
        "状态变化不应重写输入正文: {output:?}"
    );
    assert!(!output.contains("test-model"), "状态变化不应重写底栏");
}

/// 【终端】【输入回归】长输入首尾、中文软换行和小窗口始终显示光标对应行。
/// 参数: 无；返回: 无
#[test]
fn cursor_content_survives_scrolling_and_resize() {
    for (cols, rows) in [(50, 24), (20, 8), (12, 4)] {
        let mut frame = frame();
        frame.set_panel_lines(Vec::new());
        frame.input = format!("BEGIN {} END", "中文 abc\n".repeat(40));
        for cursor in [
            0,
            4,
            frame.input.chars().count() / 2,
            frame.input.chars().count(),
        ] {
            frame.cursor = cursor;
            let layout = frame.layout(cols);
            assert!(layout.cursor_row_offset < layout.input_rows);
            assert!(layout.input_rows <= REPL_MAX_VISIBLE_INPUT_ROWS);
            let mut viewport = InlineViewport::new();
            viewport.update(
                TerminalSize {
                    cols: cols as u16,
                    rows,
                },
                frame.height(cols),
                100,
            );
            let (_, signature) = frame.draw_lines(&mut Vec::new(), &viewport, None).unwrap();
            let row = usize::from(signature.cursor_row - signature.top);
            assert!(row < signature.lines.len());
            if cursor == 0 {
                assert!(signature.lines[row].contains("BEGIN"));
            }
            if cursor == frame.input.chars().count() {
                assert!(signature.lines[row].contains("END"));
            }
        }
    }
}

/// 【终端】【输入回归】满行与显式换行不重复折行，保持光标和正文一致。
/// 参数: 无；返回: 无
#[test]
fn exact_width_and_newline_share_cursor_coordinates() {
    let mut frame = frame();
    frame.set_panel_lines(Vec::new());
    for input in ["abcd\nEND", "ab中文\nEND", "a\tb\nEND"] {
        frame.input = input.into();
        frame.cursor = input.chars().count();
        let cols = 4 + crate::cli::repl_chrome::CHROME_INPUT_PREFIX_COLS;
        let layout = frame.layout(cols);
        assert!(layout.styled_display_lines[usize::from(layout.cursor_row_offset)].contains("END"));
    }
}

/// 【终端】【运行提示】运行中停止快捷键只出现在按键提示行，状态栏不再重复。
/// 参数: 无；返回: 无
#[test]
fn streaming_frame_shows_stop_shortcut_once() {
    let mut frame = frame();
    frame.set_panel_lines(Vec::new());
    frame.input.clear();
    frame.cursor = 0;
    frame.set_streaming(true);
    frame.set_key_hints(KeyHintContext {
        streaming: true,
        ..KeyHintContext::default()
    });
    let mut viewport = InlineViewport::new();
    viewport.update(
        TerminalSize {
            cols: 120,
            rows: 24,
        },
        frame.height(120),
        5,
    );
    let (_, signature) = frame.draw_lines(&mut Vec::new(), &viewport, None).unwrap();
    let plain: Vec<String> = signature
        .lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line))
        .collect();
    let stop_rows: Vec<&String> = plain
        .iter()
        .filter(|line| line.contains("Ctrl+C"))
        .collect();
    assert_eq!(stop_rows.len(), 1, "停止快捷键应只出现一次: {plain:?}");
    assert!(
        !stop_rows[0].contains("test-model"),
        "停止快捷键不应出现在状态栏: {plain:?}"
    );
}
