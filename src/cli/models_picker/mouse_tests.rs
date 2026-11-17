use super::mouse::*;
use super::render::{input_cursor_position, render_sized_with_palette, COLUMN_GAP};
use super::state::{PickerColumn, PickerState};
use crate::cli::mouse_menu::ClickTracker;
use crate::config::{AppConfig, ProviderModelChoice};
use crate::i18n::{with_locale, Locale};
use crate::render::ansi_clip::strip_ansi;
use crate::render::panel_palette::{bg, PanelPalette};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use unicode_width::UnicodeWidthStr;

/// 【终端】【鼠标样本】无参数；返回含中文名称和多个滚动窗口的模型状态。
fn state() -> PickerState {
    PickerState::new(
        (0..30)
            .map(|index| ProviderModelChoice {
                provider_id: "provider".into(),
                provider_name: "供应商".into(),
                model: format!("model-{index}"),
            })
            .collect(),
        super::THINKING_LEVELS.to_vec(),
        "provider",
        "model-20",
        "max",
    )
}

/// 【模型选择测试】【三列样本】构造启用上下文列的三列选择器状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 含五个模型与独立上下文草稿的选择状态
fn three_column_state() -> PickerState {
    let config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    let choices = (0..5)
        .map(|index| ProviderModelChoice {
            provider_id: provider_id.clone(),
            provider_name: "供应商".into(),
            model: format!("model-{index}"),
        })
        .collect();
    let mut picker = PickerState::new(
        choices,
        super::THINKING_LEVELS.to_vec(),
        &provider_id,
        "model-2",
        "high",
    );
    picker.enable_context(&config);
    picker
}

/// 【模型选择测试】【合成鼠标事件】生成无修饰键的指定坐标鼠标事件。
///
/// 参数:
/// - `kind`: 鼠标事件类型
/// - `column`: 列坐标
/// - `row`: 行坐标
///
/// 返回:
/// - 终端鼠标事件
fn mouse_at(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

/// 【终端】【窗口命中】无参数和返回值；短屏模型、思考的可见行与鼠标命中保持一致。
#[test]
fn short_picker_hits_the_rendered_window() {
    let state = state();
    let layout = PickerLayout::new(&state, 100, 10);
    assert_eq!(layout.visible, 4);
    let lines = super::render::render_sized(&state, 100, 10, false);
    for row in 0..4 {
        let index = layout.starts[0] + row;
        assert!(lines[row + 4].contains(&format!("model-{index}")));
        assert!(
            matches!(layout.hit(&state, 2, row + 4, false), Some(Target::Item(PickerColumn::Model, hit)) if hit == index)
        );
        let level = layout.starts[1] + row;
        assert!(lines[row + 4].contains(state.levels()[level]));
        assert!(
            matches!(layout.hit(&state, 51, row + 4, false), Some(Target::Item(PickerColumn::Thinking, hit)) if hit == level)
        );
    }
    assert!(layout.hit(&state, 47, 4, false).is_none());
}

/// 【终端】【窄屏列切换】无参数和返回值；隐藏列不命中，页签和保存取消在窄屏可用。
#[test]
fn narrow_picker_tabs_remain_clickable() {
    let mut state = state();
    let layout = PickerLayout::new(&state, 40, 18);
    assert!(matches!(
        layout.hit(&state, 9, 2, false),
        Some(Target::Focus(PickerColumn::Thinking))
    ));
    state.focus(PickerColumn::Thinking);
    let layout = PickerLayout::new(&state, 40, 18);
    assert!(matches!(
        layout.hit(&state, 5, 4, false),
        Some(Target::Item(PickerColumn::Thinking, 0))
    ));
    assert!(layout.hit(&state, 50, 4, false).is_none());
    assert!(matches!(
        layout.hit(&state, 2, 17, false),
        Some(Target::Key(crossterm::event::KeyCode::Enter))
    ));
}

/// 【终端】【极矮面板】无参数和返回值；一至六行时操作只命中真实显示的按钮。
#[test]
fn tiny_picker_actions_are_visible() {
    let state = state();
    for rows in 1..7 {
        let layout = PickerLayout::new(&state, 60, rows);
        let lines = super::render::render_sized(&state, 60, rows, false);
        assert_eq!(lines.len(), rows);
        let plain_footer =
            crate::cli::repl_text::strip_terminal_control_sequences(&lines[rows - 1]);
        assert!(
            plain_footer.starts_with(&format!("[{}]", crate::i18n::text("Save", "保存"))),
            "{plain_footer}"
        );
        assert!(matches!(
            layout.hit(&state, 2, rows - 1, false),
            Some(Target::Key(crossterm::event::KeyCode::Enter))
        ));
    }
}

/// 【模型选择测试】【悬停高亮】验证鼠标移过页签、三列候选与底栏按钮均更新悬停并渲染底色。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn mouse_move_highlights_tabs_columns_items_and_action_buttons() {
    let mut picker = three_column_state();
    let mut click = ClickTracker::default();
    let palette = PanelPalette::for_background((16, 18, 22));
    let hover_sgr = bg(palette.hover_bg);
    let focus_sgr = bg(palette.focus_bg);
    let width = 100usize;
    let rows = 18u16;

    // 1. 【模型选择测试】【初始焦点条】激活页签与激活列选中模型默认带焦点底色且不污染列间距
    let baseline = render_sized_with_palette(&picker, width, usize::from(rows), true, palette);
    assert!(baseline[2].contains(&focus_sgr));
    assert!(baseline[6].contains(&focus_sgr));
    assert!(!baseline[6].contains(&format!("{focus_sgr}   ")));

    // 2. 【模型选择测试】【页签悬停】移入非激活页签更新悬停目标并渲染悬停底色
    let tab_hits = tabs(&picker, width).hits().to_vec();
    assert_eq!(tab_hits.len(), 3);
    for (range, expected) in tab_hits.into_iter().skip(1) {
        let action = apply_mouse_event(
            &mut picker,
            &mut click,
            0,
            rows,
            width,
            true,
            mouse_at(MouseEventKind::Moved, range.start as u16, 2),
        );
        assert_eq!(action, MouseAction::Redraw);
        assert_eq!(picker.hovered(), Some(expected));
        let rendered = render_sized_with_palette(&picker, width, usize::from(rows), true, palette);
        assert_ne!(rendered[2], baseline[2]);
        assert!(rendered[2].contains(&hover_sgr));
        assert!(UnicodeWidthStr::width(strip_ansi(&rendered[2]).as_str()) <= width);
    }

    // 3. 【模型选择测试】【三列候选悬停】分别移过模型列、思考列与上下文列候选项均独立显示悬停底色
    let layout = PickerLayout::new(&picker, width, usize::from(rows));
    let col_x = [
        2u16,
        (layout.widths[0] + COLUMN_GAP + 1) as u16,
        (layout.widths[0] + COLUMN_GAP + layout.widths[1] + COLUMN_GAP + 1) as u16,
    ];
    let expected_targets = [
        (4u16, Target::Item(PickerColumn::Model, 0)),
        (4u16, Target::Item(PickerColumn::Thinking, 0)),
        (4u16, Target::Item(PickerColumn::Context, 0)),
    ];
    for (&x, (row, expected)) in col_x.iter().zip(expected_targets) {
        let action = apply_mouse_event(
            &mut picker,
            &mut click,
            0,
            rows,
            width,
            true,
            mouse_at(MouseEventKind::Moved, x, row),
        );
        assert_eq!(action, MouseAction::Redraw);
        assert_eq!(picker.hovered(), Some(expected));
        let rendered = render_sized_with_palette(&picker, width, usize::from(rows), true, palette);
        let idx = usize::from(row);
        assert_ne!(rendered[idx], baseline[idx]);
        assert!(rendered[idx].contains(&hover_sgr));
        assert!(UnicodeWidthStr::width(strip_ansi(&rendered[idx]).as_str()) <= width);
    }

    // 4. 【模型选择测试】【底栏按钮悬停】移过保存、取消与子任务按钮均点亮悬停样式且出界后复原
    for (range, expected) in controls(width, true).hits().to_vec() {
        let action = apply_mouse_event(
            &mut picker,
            &mut click,
            0,
            rows,
            width,
            true,
            mouse_at(MouseEventKind::Moved, range.start as u16, rows - 1),
        );
        assert_eq!(action, MouseAction::Redraw);
        assert_eq!(picker.hovered(), Some(expected));
        let rendered = render_sized_with_palette(&picker, width, usize::from(rows), true, palette);
        let footer = &rendered[usize::from(rows - 1)];
        assert_ne!(footer, &baseline[usize::from(rows - 1)]);
        assert!(footer.contains(&hover_sgr));
        assert!(UnicodeWidthStr::width(strip_ansi(footer).as_str()) <= width);
    }
    let leave = apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        rows,
        width,
        true,
        mouse_at(MouseEventKind::Moved, 2, rows + 3),
    );
    assert_eq!(leave, MouseAction::Redraw);
    assert_eq!(picker.hovered(), None);
}

/// 【模型选择测试】【显示列取字】按终端显示列定位对应起始字符。
///
/// 参数:
/// - `text`: 纯文本行
/// - `target_col`: 目标显示列坐标
///
/// 返回:
/// - 起始于该显示列的字符
fn char_at_display_col(text: &str, target_col: usize) -> Option<char> {
    let mut col = 0usize;
    for ch in text.chars() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col == target_col {
            return Some(ch);
        }
        col += width;
    }
    None
}

/// 【模型选择测试】【过滤占位防撞】验证中英文空过滤栏在光标列保留空格，非空过滤保持原紧凑格式。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn filter_placeholder_leaves_clean_cursor_cell_without_breaking_non_empty_filter() {
    let palette = PanelPalette::for_background((16, 18, 22));

    // 1. 【模型选择测试】【中文占位校验】光标停在独立空格上，不压住宽字符首字
    with_locale(Locale::Zh, || {
        let mut picker = three_column_state();
        let lines = render_sized_with_palette(&picker, 100, 18, true, palette);
        let plain = strip_ansi(&lines[1]);
        assert!(
            plain.starts_with("过滤:  输入内容过滤  ("),
            "unexpected zh placeholder: {plain}"
        );
        let (cursor_col, cursor_row) = input_cursor_position(&picker, 100, 18).unwrap();
        assert_eq!((cursor_col, cursor_row), (6, 1));
        assert_eq!(char_at_display_col(&plain, usize::from(cursor_col)), Some(' '));
        assert_eq!(char_at_display_col(&plain, usize::from(cursor_col) + 1), Some('输'));

        picker.paste_filter("manual-model");
        let typed_lines = render_sized_with_palette(&picker, 100, 18, true, palette);
        let typed_plain = strip_ansi(&typed_lines[1]);
        assert!(
            typed_plain.starts_with("过滤: manual-model  ("),
            "unexpected zh filter: {typed_plain}"
        );
    });

    // 2. 【模型选择测试】【英文占位校验】保留终端回归测试依赖的非空过滤文案格式
    with_locale(Locale::En, || {
        let mut picker = three_column_state();
        let lines = render_sized_with_palette(&picker, 100, 18, true, palette);
        let plain = strip_ansi(&lines[1]);
        assert!(
            plain.starts_with("Filter:  type to filter  ("),
            "unexpected en placeholder: {plain}"
        );
        let (cursor_col, cursor_row) = input_cursor_position(&picker, 100, 18).unwrap();
        assert_eq!((cursor_col, cursor_row), (8, 1));
        assert_eq!(char_at_display_col(&plain, usize::from(cursor_col)), Some(' '));
        assert_eq!(char_at_display_col(&plain, usize::from(cursor_col) + 1), Some('t'));

        picker.paste_filter("manual-model");
        let typed_lines = render_sized_with_palette(&picker, 100, 18, true, palette);
        let typed_plain = strip_ansi(&typed_lines[1]);
        assert!(
            typed_plain.starts_with("Filter: manual-model  ("),
            "unexpected en filter: {typed_plain}"
        );
    });
}

/// 【模型选择测试】【点击人体工学】验证点击过滤栏聚焦模型列，首次点击模型仅选中而二次点击确认。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn clicking_filter_focuses_model_and_second_click_on_active_model_confirms() {
    let mut picker = three_column_state();
    let mut click = ClickTracker::default();
    picker.focus(PickerColumn::Thinking);

    // 1. 【模型选择测试】【过滤栏点击】从思考列点击过滤栏回到模型列过滤焦点
    apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Down(MouseButton::Left), 4, 1),
    );
    let focus_action = apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Up(MouseButton::Left), 4, 1),
    );
    assert_eq!(focus_action, MouseAction::Redraw);
    assert_eq!(picker.column(), PickerColumn::Model);

    // 2. 【模型选择测试】【模型二次点击】首次点击未选中模型仅切换选中项，再次点击立即触发确认
    assert_eq!(picker.model_index(), 2);
    apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Down(MouseButton::Left), 2, 4),
    );
    let first_click = apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Up(MouseButton::Left), 2, 4),
    );
    assert_eq!(first_click, MouseAction::Redraw);
    assert_eq!(picker.model_index(), 0);

    apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Down(MouseButton::Left), 2, 4),
    );
    let second_click = apply_mouse_event(
        &mut picker,
        &mut click,
        0,
        18,
        100,
        true,
        mouse_at(MouseEventKind::Up(MouseButton::Left), 2, 4),
    );
    assert_eq!(
        second_click,
        MouseAction::Input(PickerInput::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE
        )))
    );
}
