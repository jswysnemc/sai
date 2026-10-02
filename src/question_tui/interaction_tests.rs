use super::{state::handle_editing_key, text::strip_ansi, QuestionState};
use crate::question::{parse_ask_request, validate_answers};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 【终端提问】【Other 交互回归】从 ask 参数进入卡片，编辑自定义答案并校验提交；无参数和返回值。
#[test]
fn other_is_editable_and_submittable_for_both_question_types() {
    for multiple in [false, true] {
        let request = parse_ask_request(
            &serde_json::json!({"questions":[{
                "header":"范围", "question":"选择修改范围", "multiple":multiple, "custom":false,
                "options":[{"label":"实现", "description":"修改代码"}]
            }]})
            .to_string(),
        )
        .unwrap();
        let mut state = QuestionState::new(&request);
        let frame = super::view::compose(&request, &mut state, 80, 16);
        let visible = frame
            .lines
            .iter()
            .map(|s| strip_ansi(s))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(visible.contains("2. 其他") || visible.contains("2. Other"));
        assert!(visible.contains(if multiple { "□" } else { "○" }));
        if multiple {
            state.activate_number(&request, 1).unwrap();
        }
        state.activate_number(&request, 2).unwrap();
        assert!(state.editing);
        state.edit_buffer = "补充文档".into();
        state.edit_cursor = state.edit_buffer.chars().count();
        handle_editing_key(
            &request,
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        )
        .unwrap();
        assert!(state.answers[0].contains(&"补充文档".to_string()));
        assert_eq!(state.answers[0].len(), if multiple { 2 } else { 1 });
        validate_answers(&request, &state.answers).unwrap();
        if multiple {
            // 1. 已填写的 Other 按编号切换勾选，Enter 仍可重新编辑
            state.activate_number(&request, 2).unwrap();
            assert!(!state.editing);
            assert_eq!(state.answers[0], vec!["实现"]);
            state.activate_number(&request, 2).unwrap();
            state.activate_current(&request).unwrap();
            assert!(state.editing);
            assert_eq!(state.edit_buffer, "补充文档");
            handle_editing_key(
                &request,
                &mut state,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            )
            .unwrap();
            assert!(state.answers[0].contains(&"补充文档".to_string()));
        }
    }
}

/// 【终端提问】【选择标记回归】焦点与选择状态相互独立，单选和多选使用不同标记；无参数和返回值。
#[test]
fn choice_markers_distinguish_focus_from_selection() {
    for (multiple, unselected, selected) in [(false, "○", "●"), (true, "□", "■")] {
        let focus =
            strip_ansi(&super::components::option_lines("选项", "", true, false, multiple, 40)[0]);
        let chosen =
            strip_ansi(&super::components::option_lines("选项", "", false, true, multiple, 40)[0]);
        assert!(focus.starts_with(&format!("› {unselected} ")));
        assert!(chosen.starts_with(&format!("  {selected} ")));
    }
}

/// 【终端提问】【导航回归】窄屏仍显示当前问题，已完成题使用独立标记；无参数和返回值。
#[test]
fn navigation_tracks_current_and_answered_questions() {
    let question = serde_json::json!({"header":"第一题", "question":"选范围", "options":[]});
    let mut request = parse_ask_request(
        &serde_json::json!({"questions":[question.clone(), question.clone(), question]})
            .to_string(),
    )
    .unwrap();
    request.questions[1].header = "第二题".into();
    request.questions[2].header = "第三题".into();
    let mut state = QuestionState::new(&request);
    state.answers[0] = vec!["已答".into()];
    state.tab = 1;
    let tabs = strip_ansi(&super::navigation::question_tabs(&request, &state, 80));
    assert!(tabs.contains("● 第一题"));
    assert!(tabs.contains("◆ 第二题"));
    assert!(tabs.contains("○ 第三题"));
    let narrow = strip_ansi(&super::navigation::question_tabs(&request, &state, 12));
    assert!(narrow.contains("◆ 第二题"));
}

/// 【终端提问】【单选空格回归】切换选中状态不提交、不换题，多个选项始终互斥；无参数和返回值。
#[test]
fn single_question_space_toggles_without_advancing() {
    let request = parse_ask_request(&serde_json::json!({"questions":[
        {"header":"范围","question":"选择模块","options":[{"label":"A","description":""},{"label":"B","description":""}]},
        {"header":"确认","question":"补充说明","options":[]}
    ]}).to_string()).unwrap();
    let mut state = QuestionState::new(&request);
    state.toggle_current(&request).unwrap();
    assert_eq!(state.tab, 0, "空格只能切换，不能推进到下一题");
    assert_eq!(state.answers[0], vec!["A"]);
    state.selected[0] = 1;
    state.toggle_current(&request).unwrap();
    assert_eq!(state.answers[0], vec!["B"]);
    state.toggle_current(&request).unwrap();
    assert!(state.answers[0].is_empty());
    state.activate_current(&request).unwrap();
    assert_eq!(state.tab, 1);
    assert_eq!(state.answers[0], vec!["B"]);
}

/// 【终端提问】【确认回归】Enter 保留空格选中的答案，自定义答案也满足互斥与取消选择；无参数和返回值。
#[test]
fn single_question_confirm_preserves_choice_and_other_toggle() {
    let request = parse_ask_request(&serde_json::json!({"questions":[{
        "header":"范围","question":"选择模块","options":[{"label":"A","description":""},{"label":"B","description":""}]
    }]}).to_string()).unwrap();
    let mut state = QuestionState::new(&request);
    state.toggle_current(&request).unwrap();
    state.selected[0] = 1;
    state.confirm_single(&request).unwrap();
    assert_eq!(state.answers[0], vec!["A"]);
    state.selected[0] = 2;
    state.confirm_single(&request).unwrap();
    assert!(state.editing);
    state.edit_buffer = "自定义".into();
    handle_editing_key(
        &request,
        &mut state,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
    .unwrap();
    assert_eq!(state.answers[0], vec!["自定义"]);
    state.toggle_current(&request).unwrap();
    assert!(state.answers[0].is_empty());
    state.toggle_current(&request).unwrap();
    assert_eq!(state.answers[0], vec!["自定义"]);
    state.selected[0] = 0;
    state.toggle_current(&request).unwrap();
    assert_eq!(state.answers[0], vec!["A"]);
}
