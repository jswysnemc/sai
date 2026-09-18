use super::styled_line::wrap_styled_line;
use super::*;
use crate::agent::AgentMode;
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
        activity: None,
        status_plugin: None,
    };
    let mut frame = ComposerFrame::new(chrome, "draft".into(), 5, false, Vec::new(), 0);
    frame.set_panel_lines(vec!["Working 1s".into()]);
    frame
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
