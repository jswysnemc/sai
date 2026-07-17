use super::render::{editor_option_line, option_lines, panel_layout};
use super::text::{editor_view, insert_text, strip_ansi, truncate_width};
use super::*;
use crate::question::QuestionOption;
use crate::question::MAX_CUSTOM_ANSWER_CHARS;
use unicode_width::UnicodeWidthStr;

fn multi_request() -> QuestionRequest {
    QuestionRequest {
        questions: vec![QuestionPrompt {
            header: "范围".to_string(),
            question: "选择范围".to_string(),
            options: vec![
                QuestionOption {
                    label: "代码".to_string(),
                    description: String::new(),
                },
                QuestionOption {
                    label: "文档".to_string(),
                    description: String::new(),
                },
            ],
            multiple: true,
            custom: true,
        }],
    }
}

#[test]
fn multi_activation_toggles_selected_option() {
    let request = multi_request();
    let mut state = QuestionState::new(&request);
    state.activate_current(&request).unwrap();
    assert_eq!(state.answers[0], vec!["代码"]);
    state.activate_current(&request).unwrap();
    assert!(state.answers[0].is_empty());
}

#[test]
fn left_and_right_cycle_question_tabs() {
    let mut request = multi_request();
    request.questions.push(request.questions[0].clone());
    let mut state = QuestionState::new(&request);
    state.next_tab(&request);
    assert_eq!(state.tab, 1);
    state.previous_tab(&request);
    assert_eq!(state.tab, 0);
}

#[test]
fn custom_input_is_sanitized_and_bounded() {
    let mut value = String::new();
    let mut cursor = 0;
    let input = format!("a\u{1b}\t{}", "b".repeat(MAX_CUSTOM_ANSWER_CHARS));
    insert_text(&mut value, &mut cursor, &input);
    assert!(!value.contains('\u{1b}'));
    assert!(!value.contains('\t'));
    assert_eq!(value.chars().count(), MAX_CUSTOM_ANSWER_CHARS);
}

#[test]
fn editor_view_keeps_caret_visible() {
    let (view, cursor) = editor_view("abcdefghijkl", 10, 6);
    assert!(view.starts_with('…'));
    assert!(cursor <= 6);
    assert!(UnicodeWidthStr::width(view.as_str()) <= 6);
}

#[test]
fn final_answer_waits_on_review_tab() {
    let mut request = multi_request();
    request.questions[0].multiple = false;
    request.questions[0].custom = false;
    request.questions.push(request.questions[0].clone());
    let mut state = QuestionState::new(&request);
    state.activate_current(&request).unwrap();
    state.activate_current(&request).unwrap();
    assert!(state.on_confirm(&request));
    assert!(submitted_answers(&request, &state).unwrap().is_some());
}

#[test]
fn existing_custom_answer_reopens_for_editing() {
    let request = QuestionRequest {
        questions: vec![QuestionPrompt {
            header: "范围".to_string(),
            question: "选择范围".to_string(),
            options: Vec::new(),
            multiple: false,
            custom: true,
        }],
    };
    let mut state = QuestionState::new(&request);
    state.custom_answers[0] = "已有答案".to_string();
    state.answers[0] = vec!["已有答案".to_string()];
    state.activate_current(&request).unwrap();
    assert!(state.editing);
    assert_eq!(state.edit_buffer, "已有答案");
}

#[test]
fn existing_multi_custom_answer_can_be_toggled_off() {
    let mut request = multi_request();
    let mut state = QuestionState::new(&request);
    state.selected[0] = request.questions[0].options.len();
    state.custom_answers[0] = "已有答案".to_string();
    state.answers[0] = vec!["已有答案".to_string()];
    state.toggle_current(&request).unwrap();
    assert!(state.answers[0].is_empty());

    request.questions[0].multiple = false;
    state.activate_current(&request).unwrap();
    assert!(state.editing);
}

#[test]
fn option_rows_have_no_numbers_and_put_description_below_title() {
    let lines = option_lines("烧烤", "烤肉串、烤鸡翅、烤韭菜", true, false, false, 16);
    let visible = lines
        .iter()
        .map(|line| strip_ansi(line))
        .collect::<Vec<_>>();
    assert_eq!(visible[0], "> 烧烤");
    assert!(!visible.iter().any(|line| line.contains("1.")));
    assert!(visible[1..].iter().all(|line| line.starts_with("  ")));
    assert!(lines[1..].iter().all(|line| line.contains("\x1b[2m")));
}

#[test]
fn multi_option_rows_keep_checkbox_without_number() {
    let lines = option_lines("代码", "修改实现和测试", true, true, true, 18);
    assert_eq!(strip_ansi(&lines[0]), "> [x] 代码");
    assert!(strip_ansi(&lines[1]).starts_with("      "));
}

#[test]
fn description_soft_wrap_preserves_indentation_budget() {
    let lines = option_lines("烧烤", "烤肉串烤鸡翅烤韭菜", false, false, false, 10);
    assert!(lines.len() > 2);
    for line in &lines[1..] {
        assert!(UnicodeWidthStr::width(strip_ansi(line).as_str()) <= 10);
        assert!(strip_ansi(line).starts_with("  "));
    }
}

#[test]
fn resize_recovers_panel_height_after_terminal_grows() {
    let mut session = std::mem::ManuallyDrop::new(QuestionSession {
        stdout: io::stdout(),
        anchor_y: 8,
        panel_lines: 12,
    });
    session.resize_to_terminal(3);
    assert_eq!(session.panel_lines, 2);
    session.resize_to_terminal(24);
    assert_eq!(session.panel_lines, MAX_PANEL_LINES);
}

#[test]
fn truncation_honors_very_narrow_widths() {
    assert_eq!(truncate_width("abcdef", 1), ".");
    assert_eq!(truncate_width("abcdef", 2), "..");
    assert_eq!(
        UnicodeWidthStr::width(truncate_width("中文测试", 3).as_str()),
        3
    );
}

#[test]
fn selected_option_uses_color_without_bold() {
    let lines = option_lines("烧烤", "", true, false, false, 20);
    assert!(lines[0].contains("\x1b[35m"));
    assert!(!lines[0].contains("\x1b[1m"));
}

#[test]
fn custom_editor_has_no_extra_ascii_pointer() {
    let line = editor_option_line(false, false, "自定义内容");
    assert_eq!(strip_ansi(&line), "> 自定义内容");
}

#[test]
fn ctrl_j_inserts_custom_answer_newline() {
    let request = QuestionRequest {
        questions: vec![QuestionPrompt {
            header: "说明".to_string(),
            question: "补充说明".to_string(),
            options: Vec::new(),
            multiple: false,
            custom: true,
        }],
    };
    let mut state = QuestionState::new(&request);
    state.editing = true;
    state.edit_buffer = "前".to_string();
    state.edit_cursor = 1;
    handle_editing_key(
        &request,
        &mut state,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
    )
    .unwrap();
    assert_eq!(state.edit_buffer, "前\n");
}

#[test]
fn scrolling_only_changes_body_window() {
    let first = panel_layout(3, 30, 1, 16, Some(0), 0);
    let last = panel_layout(3, 30, 1, 16, Some(29), first.body_start);
    assert_eq!(first.top_budget, 3);
    assert_eq!(last.top_budget, 3);
    assert_eq!(first.footer_start, 0);
    assert_eq!(last.footer_start, 0);
    assert_eq!(first.body_capacity, 12);
    assert_ne!(first.body_start, last.body_start);
}

#[test]
fn scrolling_waits_until_focus_crosses_viewport_edge() {
    let inside = panel_layout(2, 12, 1, 8, Some(4), 0);
    assert_eq!(inside.body_capacity, 5);
    assert_eq!(inside.body_start, 0);

    let below = panel_layout(2, 12, 1, 8, Some(5), inside.body_start);
    assert_eq!(below.body_start, 1);

    let still_inside = panel_layout(2, 12, 1, 8, Some(4), below.body_start);
    assert_eq!(still_inside.body_start, 1);

    let above = panel_layout(2, 12, 1, 8, Some(0), still_inside.body_start);
    assert_eq!(above.body_start, 0);
}
