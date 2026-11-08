use super::context::{ContextSelection, OPTION_COUNT};
use super::render::render_with_width;
use super::state::{PickerColumn, PickerState};
use crate::config::{AppConfig, ProviderModelChoice};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthStr;

/// 【终端】【上下文测试】构造两个模型，分别配置 API 容量和独立上限。
/// 参数: 无；返回: 已启用上下文列的模型选择状态
fn picker() -> PickerState {
    let mut config = AppConfig::default();
    let provider = &mut config.providers[0];
    provider.set_model_context_chars_for("model-one", Some(1_000_000));
    provider.set_model_context_limit_tokens_for("model-one", Some(300_000));
    provider.set_model_context_chars_for("model-two", Some(128_000));
    let choices = ["model-one", "model-two"]
        .into_iter()
        .map(|model| ProviderModelChoice {
            provider_id: provider.id.clone(),
            provider_name: "Provider".to_string(),
            model: model.to_string(),
        })
        .collect();
    let mut picker = PickerState::new(
        choices,
        vec!["auto", "high"],
        &provider.id,
        "model-one",
        "auto",
    );
    picker.enable_context(&config);
    picker
}

/// 【终端】【上下文测试】模拟在当前上下文选择中输入字符串。
/// 参数: `selection` 为选择草稿，`input` 为输入内容；返回: 无
fn type_input(selection: &mut ContextSelection, input: &str) {
    for character in input.chars() {
        assert!(selection.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)));
    }
}

/// 【终端】【上下文测试】验证六个选项及三列焦点移动。
/// 参数: 无；返回: 无
#[test]
fn presets_and_focus_navigation_cover_all_three_columns() {
    let mut picker = picker();
    picker.focus_right();
    assert_eq!(picker.column(), PickerColumn::Thinking);
    picker.focus_right();
    picker.focus_right();
    assert_eq!(picker.column(), PickerColumn::Context);
    picker.move_up();
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        Some(200_000)
    );
    for expected in [300_000, 400_000, 500_000] {
        picker.move_down();
        assert_eq!(
            picker
                .context_selection()
                .unwrap()
                .selected_limit()
                .unwrap(),
            Some(expected)
        );
    }
    for _ in 0..OPTION_COUNT {
        picker.move_down();
    }
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        None
    );
    picker.focus_left();
    assert_eq!(picker.column(), PickerColumn::Thinking);
    picker.focus_left();
    picker.focus_left();
    assert_eq!(picker.column(), PickerColumn::Model);
    assert_eq!(picker.selected_model().unwrap().model, "model-one");
    assert_eq!(picker.selected_level(), "auto");
}

/// 【终端】【上下文测试】验证模型切换和过滤恢复时保留独立草稿。
/// 参数: 无；返回: 无
#[test]
fn context_drafts_follow_provider_model_selection() {
    let mut picker = picker();
    picker.focus_right();
    picker.focus_right();
    picker.move_down();
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        Some(400_000)
    );
    picker.focus_model();
    picker.move_down();
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        None
    );
    assert!(picker
        .context_selection()
        .unwrap()
        .status()
        .contains("128k"));
    picker.move_up();
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        Some(400_000)
    );
    picker.push_filter('z');
    assert!(picker.context_selection().is_none());
    picker.clear_filter();
    assert_eq!(
        picker
            .context_selection()
            .unwrap()
            .selected_limit()
            .unwrap(),
        Some(400_000)
    );
}

/// 【终端】【上下文测试】验证自定义输入支持单位、退格和清空且不修改过滤条件。
/// 参数: 无；返回: 无
#[test]
fn custom_input_is_separate_from_model_filter() {
    let mut picker = picker();
    let selection = picker.context_selection_mut().unwrap();
    type_input(selection, "350k");
    assert_eq!(selection.selected_limit().unwrap(), Some(350_000));
    assert!(selection.validate());
    selection.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert_eq!(selection.selected_limit().unwrap(), Some(350));
    selection.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert!(!selection.validate());
    type_input(selection, "0.4m");
    assert_eq!(selection.selected_limit().unwrap(), Some(400_000));
    assert!(picker.filter().is_empty());
}

/// 【终端】【上下文测试】验证无效、零值、溢出输入不允许保存，并可切回最大。
/// 参数: 无；返回: 无
#[test]
fn invalid_input_is_reported_without_losing_the_maximum_option() {
    for input in ["", "0", ".", "1kk", "99999999999999999999999999999999k"] {
        let mut selection = ContextSelection::new(None, Some(1_000_000), 128_000);
        selection.move_selection(false);
        type_input(&mut selection, input);
        assert!(!selection.validate(), "invalid input: {input}");
        assert!(selection.status().contains("350k"));
        selection.move_selection(true);
        assert!(selection.validate());
        assert_eq!(selection.selected_limit().unwrap(), None);
    }
}

/// 【终端】【上下文测试】验证自定义值初始化及超过模型容量时展示实际预算。
/// 参数: 无；返回: 无
#[test]
fn custom_values_and_capacity_caps_are_visible() {
    let mut selection = ContextSelection::new(Some(350_123), Some(256_000), 128_000);
    assert_eq!(selection.selected_limit().unwrap(), Some(350_123));
    assert!(selection
        .label(selection.index())
        .unwrap()
        .contains("350123"));
    assert!(selection.status().contains("256k"));
    selection.move_selection(true);
    assert!(selection.label(selection.index()).unwrap().contains("256k"));
    assert_eq!(selection.selected_limit().unwrap(), None);
}

/// 【终端】【上下文测试】验证三列高亮、预设展示和窄终端显示宽度。
/// 参数: 无；返回: 无
#[test]
fn rendering_fits_wide_and_narrow_terminals() {
    let mut picker = picker();
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    for _ in 0..3 {
        for width in [0, 1, 20, 44, 69, 70, 80, 100, 140] {
            let lines = render_with_width(&picker, width);
            assert_eq!(lines.len(), super::render::frame_row_count() as usize);
            for line in &lines {
                let plain = ansi.replace_all(line, "");
                assert!(
                    UnicodeWidthStr::width(plain.as_ref()) <= width,
                    "width {width}: {plain}"
                );
            }
            if width >= 70 {
                let rendered = lines.join("\n");
                assert_eq!(rendered.matches('›').count(), 1);
                for label in ["200k", "300k", "400k", "500k", "1m"] {
                    assert!(rendered.contains(label), "missing {label}");
                }
            }
        }
        picker.focus_right();
    }
}

/// 【终端】【上下文测试】验证未启用第三列的子任务选择器保持两列导航。
/// 参数: 无；返回: 无
#[test]
fn subagent_picker_keeps_two_columns() {
    let mut picker = PickerState::new(Vec::new(), vec!["auto"], "", "", "auto");
    picker.focus_right();
    picker.focus_right();
    assert_eq!(picker.column(), PickerColumn::Thinking);
    assert!(!picker.context_enabled());
    assert!(picker.context_selection().is_none());
}
