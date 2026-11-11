use super::mouse::*;
use super::state::{PickerColumn, PickerState};
use crate::config::ProviderModelChoice;

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
        assert!(lines[rows - 1].starts_with(&format!("[{}]", crate::i18n::text("Save", "保存"))));
        assert!(matches!(
            layout.hit(&state, 2, rows - 1, false),
            Some(Target::Key(crossterm::event::KeyCode::Enter))
        ));
    }
}
