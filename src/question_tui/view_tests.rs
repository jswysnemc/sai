use super::text::strip_ansi;
use super::*;
use crate::question::QuestionOption;
use unicode_width::UnicodeWidthStr;

/// 【终端提问】【测试请求】无参数，返回包含中文说明的多选问题。
fn request() -> QuestionRequest {
    QuestionRequest {
        questions: vec![QuestionPrompt {
            header: "修改范围".into(),
            question: "这次需要处理哪些内容？".into(),
            options: vec![
                QuestionOption {
                    label: "修改实现与测试".into(),
                    description: "修复行为，并补充回归测试。".into(),
                    value: None,
                },
                QuestionOption {
                    label: "更新使用文档".into(),
                    description: "说明新的操作方式和适用范围。".into(),
                    value: None,
                },
            ],
            multiple: true,
            custom: true,
            required: true,
            default_answers: vec![],
            validation: None,
        }],
    }
}

/// 【终端提问】【页面回归】标题、勾选状态与当前说明分层显示；无参数或返回值。
#[test]
fn question_frame_shows_progress_and_only_focused_description() {
    let request = request();
    let mut state = QuestionState::new(&request);
    state.toggle_current(&request).unwrap();
    let frame = super::view::compose(&request, &mut state, 80, 12);
    let plain = frame
        .lines
        .iter()
        .map(|line| strip_ansi(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("1/1"));
    assert!(plain.contains("> [x] 修改实现与测试"));
    assert!(plain.contains("修复行为，并补充回归测试。"));
    assert!(!plain.contains("说明新的操作方式和适用范围。"));
    assert!(plain.contains("更新使用文档"));
    println!("{plain}");
}

/// 【终端提问】【小窗口回归】各种尺寸下可见行和光标都不越界；无参数或返回值。
#[test]
fn narrow_frames_keep_editor_and_rows_inside_terminal() {
    let request = request();
    for cols in [1, 2, 8, 24, 40, 80] {
        for rows in [1, 2, 4, 8, 16] {
            let mut state = QuestionState::new(&request);
            state.selected[0] = 2;
            state.activate_current(&request).unwrap();
            state.edit_buffer = "中文自定义答案和完整上下文".repeat(4);
            state.edit_cursor = state.edit_buffer.chars().count();
            let frame = super::view::compose(&request, &mut state, cols, rows);
            assert_eq!(frame.lines.len(), rows);
            assert!(frame
                .lines
                .iter()
                .all(|line| UnicodeWidthStr::width(strip_ansi(line).as_str()) <= cols));
            let (col, row) = frame.cursor.expect("编辑器应始终保留在可见区域");
            assert!(col < cols && row < rows);
        }
    }
}

/// 【终端提问】【多选回归】继续不会隐式勾选，确认页仍拦截未回答的必答题；无参数或返回值。
#[test]
fn continuing_multiple_selection_does_not_toggle_answers() {
    let request = request();
    let mut state = QuestionState::new(&request);
    state.continue_multiple(&request).unwrap();
    assert!(state.on_confirm(&request));
    assert!(state.answers[0].is_empty());
    assert!(submitted_answers(&request, &state).unwrap().is_none());
    state.previous_tab(&request);
    state.toggle_current(&request).unwrap();
    state.continue_multiple(&request).unwrap();
    assert_eq!(
        submitted_answers(&request, &state).unwrap().unwrap()[0],
        vec!["修改实现与测试"]
    );
}

/// 【终端提问】【默认焦点】默认答案与初始焦点一致，避免 Enter 意外覆盖默认项；无参数或返回值。
#[test]
fn initial_focus_matches_default_answer() {
    let mut request = request();
    request.questions[0].default_answers = vec!["更新使用文档".into()];
    let state = QuestionState::new(&request);
    assert_eq!(state.selected[0], 1);
}
