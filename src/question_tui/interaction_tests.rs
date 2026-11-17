use super::mouse::{MouseOutcome, MouseTarget, QuestionMouse};
use super::state::{handle_editing_key, QuestionState};
use super::text::strip_ansi;
use crate::question::{parse_ask_request, validate_answers, QuestionRequest};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

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
            // 1. 【终端提问】【Other 切换】已填写的 Other 按编号切换勾选，Enter 仍可重新编辑
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

/// 【终端提问】【选择标记回归】单选使用圆点、多选使用方框，且焦点与选择状态相互独立。
#[test]
fn choice_markers_distinguish_question_kind_and_focus() {
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

/// 【终端提问】【改选确认回归】移动焦点后按 Enter 立即更新为新高亮选项，不再锁死旧答案。
#[test]
fn single_question_confirm_updates_to_newly_focused_choice_and_supports_other() {
    let request = parse_ask_request(&serde_json::json!({"questions":[{
        "header":"范围","question":"选择模块","options":[{"label":"A","description":""},{"label":"B","description":""}]
    }]}).to_string()).unwrap();
    let mut state = QuestionState::new(&request);
    state.toggle_current(&request).unwrap();
    assert_eq!(state.answers[0], vec!["A"]);
    state.selected[0] = 1;
    state.confirm_single(&request).unwrap();
    assert_eq!(state.answers[0], vec!["B"]);

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

/// 【终端提问】【草稿保护回归】在自定义编辑态按一次 Esc 仅退出编辑，长文本草稿不丢失。
#[test]
fn escape_in_editing_mode_preserves_unsaved_draft() {
    let request = parse_ask_request(&serde_json::json!({"questions":[{
        "header":"说明","question":"输入方案","options":[{"label":"默认","description":""}]
    }]}).to_string()).unwrap();
    let mut state = QuestionState::new(&request);
    state.selected[0] = 1;
    state.activate_current(&request).unwrap();
    assert!(state.editing);

    state.edit_buffer = "一段还没写完的详细方案草稿".into();
    state.edit_cursor = state.edit_buffer.chars().count();
    handle_editing_key(
        &request,
        &mut state,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .unwrap();
    assert!(!state.editing);
    assert_eq!(state.edit_buffer, "一段还没写完的详细方案草稿");

    state.activate_current(&request).unwrap();
    assert!(state.editing);
    assert_eq!(state.edit_buffer, "一段还没写完的详细方案草稿");
}

/// 【终端提问】【鼠标交互回归】题签点击换题、选项单击聚焦/二次点击激活、滚轮滚动与确认页点击提交。
#[test]
fn mouse_switches_tabs_selects_options_scrolls_and_submits_review() {
    let request = parse_ask_request(&serde_json::json!({"questions":[
        {"header":"第一题","question":"选方向","options":[
            {"label":"甲","description":""},
            {"label":"乙","description":""},
            {"label":"丙","description":""},
            {"label":"丁","description":""},
            {"label":"戊","description":""},
            {"label":"己","description":""},
            {"label":"庚","description":""},
            {"label":"辛","description":""}
        ]},
        {"header":"第二题","question":"选测试","multiple":true,"options":[
            {"label":"单元","description":""},
            {"label":"集成","description":""}
        ]}
    ]}).to_string()).unwrap();
    let mut state = QuestionState::new(&request);
    let mut mouse = QuestionMouse::default();

    let click_at = |mouse: &mut QuestionMouse,
                    hits: &super::mouse::QuestionHitMap,
                    request: &QuestionRequest,
                    state: &mut QuestionState,
                    col: u16,
                    row: u16| {
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::NONE,
        };
        mouse.handle(down, 0, hits, request, state).unwrap();
        mouse.handle(up, 0, hits, request, state).unwrap()
    };

    // 1. 滚轮向下滚动长选项列表
    let frame = super::view::compose(&request, &mut state, 80, 12);
    assert!(frame.hits.max_body_scroll > 0);
    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 4,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        mouse.handle(wheel, 0, &frame.hits, &request, &mut state).unwrap(),
        MouseOutcome::Redraw
    );
    assert!(state.scroll_starts[0] > 0);

    // 2. 点击题签切换到第二题，再点回第一题
    let frame = super::view::compose(&request, &mut state, 80, 12);
    let (tab_row, tab_cols, _) = frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::Tab(1))
        .unwrap()
        .clone();
    click_at(
        &mut mouse,
        &frame.hits,
        &request,
        &mut state,
        tab_cols.start as u16,
        tab_row as u16,
    );
    assert_eq!(state.tab, 1);

    let frame = super::view::compose(&request, &mut state, 80, 12);
    let (tab0_row, tab0_cols, _) = frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::Tab(0))
        .unwrap()
        .clone();
    click_at(
        &mut mouse,
        &frame.hits,
        &request,
        &mut state,
        tab0_cols.start as u16,
        tab0_row as u16,
    );
    assert_eq!(state.tab, 0);

    // 3. 单选题：第一次点击“乙”移动焦点，第二次点击“乙”选定并推进到第二题
    let frame = super::view::compose(&request, &mut state, 80, 16);
    let (opt1_row, _, _) = frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::Option(1))
        .unwrap()
        .clone();
    click_at(&mut mouse, &frame.hits, &request, &mut state, 4, opt1_row as u16);
    assert_eq!(state.selected[0], 1);
    assert_eq!(state.tab, 0);
    click_at(&mut mouse, &frame.hits, &request, &mut state, 4, opt1_row as u16);
    assert_eq!(state.answers[0], vec!["乙"]);
    assert_eq!(state.tab, 1);

    // 4. 多选题：点击已聚焦的第 0 项直接勾选，点击确认题签进入确认页并点击提交
    let frame = super::view::compose(&request, &mut state, 80, 16);
    let (m0_row, _, _) = frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::Option(0))
        .unwrap()
        .clone();
    click_at(&mut mouse, &frame.hits, &request, &mut state, 4, m0_row as u16);
    assert_eq!(state.answers[1], vec!["单元"]);

    let frame = super::view::compose(&request, &mut state, 80, 16);
    let (rev_row, rev_cols, _) = frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::Tab(2))
        .unwrap()
        .clone();
    click_at(
        &mut mouse,
        &frame.hits,
        &request,
        &mut state,
        rev_cols.start as u16,
        rev_row as u16,
    );
    assert!(state.on_confirm(&request));

    let confirm_frame = super::view::compose(&request, &mut state, 80, 16);
    let (sub_row, sub_cols, _) = confirm_frame
        .hits
        .regions
        .iter()
        .find(|(_, _, t)| *t == MouseTarget::ConfirmSubmit)
        .unwrap()
        .clone();
    let outcome = click_at(
        &mut mouse,
        &confirm_frame.hits,
        &request,
        &mut state,
        sub_cols.start as u16,
        sub_row as u16,
    );
    assert_eq!(outcome, MouseOutcome::SubmitIfReady);
}
