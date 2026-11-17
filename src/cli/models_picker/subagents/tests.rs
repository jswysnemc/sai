use super::*;
use crate::config::AgentProfile;

/// 构造可委派档案；参数为标识和注册状态，返回独立档案。
fn profile(id: &str, registered: bool) -> AgentProfile {
    AgentProfile {
        id: id.to_string(),
        name: id.to_string(),
        register_to_main: registered,
        ..AgentProfile::default()
    }
}

/// 共享设置始终排在第一项，列表只包含可以委派的类型。
#[test]
fn list_contains_shared_defaults_and_registered_profiles() {
    let mut config = AppConfig::default();
    config.agents.push(profile("worker", true));
    config.agents.push(profile("main-only", false));
    let state = SubagentListState::new(&config, None);
    assert!(state.selected().profile_id.is_none());
    assert!(state
        .targets
        .iter()
        .any(|target| target.profile_id.as_deref() == Some("worker")));
    assert!(!state
        .targets
        .iter()
        .any(|target| target.profile_id.as_deref() == Some("main-only")));
}

/// 列表恢复上次类型，未知类型回到共享设置；移动不会越界。
#[test]
fn list_restores_selection_and_clamps_navigation() {
    let config = AppConfig::default();
    let state = SubagentListState::new(&config, Some("explore"));
    assert_eq!(state.selected().profile_id.as_deref(), Some("explore"));
    let mut state = SubagentListState::new(&config, Some("missing"));
    state.move_up();
    assert_eq!(state.index, 0);
    for _ in 0..state.targets.len() + 3 {
        state.move_down();
    }
    assert_eq!(state.index, state.targets.len() - 1);
}

/// 共享模型可以继承主对话，单类型选择器定位到已保存的模型和思考。
#[test]
fn choice_state_restores_independent_model_and_thinking() {
    let mut config = AppConfig::default();
    let shared = SubagentListState::new(&config, None);
    let picker = choice_state(&config, shared.selected());
    assert!(picker.selected_model().unwrap().provider_id.is_empty());
    let provider = config.providers[0].clone();
    config
        .set_subagent_model_choice(
            Some("explore"),
            SubagentModelChoice {
                provider_id: provider.id.clone(),
                model: provider.default_model.clone(),
                thinking_level: "high".to_string(),
            },
        )
        .unwrap();
    let state = SubagentListState::new(&config, Some("explore"));
    let picker = choice_state(&config, state.selected());
    assert_eq!(
        picker.selected_model().unwrap().model,
        provider.default_model
    );
    assert_eq!(picker.selected_level(), "high");
}

/// 【子任务设置测试】【80列排版】长供应商与模型名称在 80 列终端中压缩中间列，右侧思考等级始终完整保留。
#[test]
fn eighty_column_subagent_row_preserves_right_aligned_thinking_level() {
    let mut config = AppConfig::default();
    config.providers[0].id = "openrouter-enterprise".into();
    config.providers[0].models =
        vec!["anthropic/claude-3.7-sonnet-thinking-preview-2026".into()];
    config.providers[0].default_model = config.providers[0].models[0].clone();
    config.active_provider = "openrouter-enterprise".into();
    config
        .set_subagent_model_choice(
            None,
            SubagentModelChoice {
                provider_id: "openrouter-enterprise".into(),
                model: "anthropic/claude-3.7-sonnet-thinking-preview-2026".into(),
                thinking_level: "xhigh".into(),
            },
        )
        .unwrap();
    let state = SubagentListState::new(&config, None);
    let lines = render::list(&state, 79, 12, "");
    let plain = crate::cli::repl_text::strip_terminal_control_sequences(&lines[3]);
    assert!(
        plain.ends_with("·  xhigh"),
        "右侧思考等级不应被长模型名截断: {plain}"
    );
    assert!(unicode_width::UnicodeWidthStr::width(plain.as_str()) <= 79);
}

/// 【子任务设置测试】【鼠标悬停与二次点击】验证子任务列表悬停行、悬停返回按钮及二次点击进入编辑。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn subagent_list_highlights_hovered_controls_and_confirms_on_second_click() {
    use crate::render::ansi_clip::strip_ansi;
    use crate::render::panel_palette::{bg, PanelPalette};
    use crossterm::event::MouseButton;

    let mut config = AppConfig::default();
    config.agents.push(profile("worker", true));
    let mut state = SubagentListState::new(&config, None);
    let mut click = ClickTracker::default();
    let palette = PanelPalette::for_background((16, 18, 22));
    let hover_sgr = bg(palette.hover_bg);
    let focus_sgr = bg(palette.focus_bg);

    // 1. 【子任务设置测试】【默认焦点底色】初始选中首行带焦点底色，次行与底栏无悬停底色
    let baseline = render::list_with_palette(&state, 80, 12, "", palette);
    assert!(baseline[3].contains(&focus_sgr));
    assert!(!baseline[4].contains(&hover_sgr));
    assert!(!baseline[11].contains(&hover_sgr));

    // 2. 【子任务设置测试】【行悬停反馈】移入未选中子任务行时渲染悬停底色
    let move_row = handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(move_row, SubagentMouseAction::Redraw);
    assert_eq!(state.hovered(), Some(ListTarget::Item(1)));
    let row_hovered = render::list_with_palette(&state, 80, 12, "", palette);
    assert!(row_hovered[4].contains(&hover_sgr));
    assert!(unicode_width::UnicodeWidthStr::width(strip_ansi(&row_hovered[4]).as_str()) <= 80);

    // 3. 【子任务设置测试】【返回按钮悬停】移入底栏返回按钮时点亮悬停底色
    let move_back = handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 2,
            row: 11,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(move_back, SubagentMouseAction::Redraw);
    assert_eq!(state.hovered(), Some(ListTarget::Back));
    let back_hovered = render::list_with_palette(&state, 80, 12, "", palette);
    assert!(back_hovered[11].contains(&hover_sgr));
    assert!(unicode_width::UnicodeWidthStr::width(strip_ansi(&back_hovered[11]).as_str()) <= 80);

    // 4. 【子任务设置测试】【二次点击进入】未选中行首次点击先切换焦点，再次点击进入设置
    handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    let first = handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(first, SubagentMouseAction::Redraw);
    assert_eq!(state.index, 1);

    handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    let second = handle_list_mouse(
        &mut state,
        &mut click,
        0,
        12,
        80,
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: 4,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(
        second,
        SubagentMouseAction::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    );
}
