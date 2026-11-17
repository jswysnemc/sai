//! 通用配置表单控制器：支持 Tab/BackTab 环形焦点切换、粘贴录入、只读保护及外部编辑器集成。

mod detail;
mod field;
mod helpers;
mod mouse;
mod render;

pub(crate) use field::Field;
pub(crate) use helpers::{
    add_custom_model_form, edit_textarea, parse_bool_field, parse_number_field,
    parse_provider_model_choice, provider_model_choice_values, vision_provider_value,
};
use helpers::{choice_label, select_choice};
use mouse::{apply_form_mouse, form_hover_target, wait_form_non_hover_key, FormEventStep};
use render::draw_form_interactive;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers, MouseEventKind};
use std::io;

use super::input::{read_event, sanitize_paste_single_line, ConfigEvent, UnsavedState};
use super::mouse::ClickTracker;

/// 表单内文本编辑期间的输入法状态机。
pub(super) struct FcitxState;

impl FcitxState {
    /// 【配置表单】【输入法初始化】进入表单导航时默认关闭输入法，避免快捷键被候选框拦截。
    pub(super) fn new() -> Self {
        super::ime::disable_for_nav();
        Self
    }

    /// 【配置表单】【开启输入法】进入单行文本框编辑时恢复用户原输入法状态。
    pub(super) fn enter_editing(&mut self) {
        super::ime::enable_for_edit();
    }

    /// 【配置表单】【关闭输入法】退出单行文本框回到字段导航时关闭输入法。
    pub(super) fn leave_editing(&mut self) {
        super::ime::disable_for_nav();
    }
}

impl Drop for FcitxState {
    fn drop(&mut self) {
        super::ime::disable_for_nav();
    }
}

/// 表单行内快捷循环切换的最大候选数；超出时提供可检索完整列表。
pub(super) const INLINE_CHOICE_LIMIT: usize = 5;

/// 【配置表单】【运行表单】驱动字段焦点导航、行内编辑、粘贴及保存/取消按钮交互。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 表单顶栏标题
/// - `fields`: 表单字段可变切片
///
/// 返回:
/// - 用户确认保存时返回 `Ok(true)`；取消返回 `Ok(false)`
pub(crate) fn run_form(stdout: &mut io::Stdout, title: &str, fields: &mut [Field]) -> Result<bool> {
    let mut selected = first_selectable(fields);
    let mut hovered: Option<usize> = None;
    let mut footer_hover: Option<super::mouse::HitRect> = None;
    let mut editing = false;
    let mut fcitx = FcitxState::new();
    let mut clicks = ClickTracker::default();
    let baseline_values = fields
        .iter()
        .map(|field| field.value.clone())
        .collect::<Vec<_>>();
    let mut unsaved_guard = UnsavedState::default();
    let mut cursors = fields
        .iter()
        .map(|field| field.value.chars().count())
        .collect::<Vec<_>>();
    let mut revealed_secrets = vec![false; fields.len()];
    loop {
        let form_dirty = fields
            .iter()
            .zip(&baseline_values)
            .any(|(field, base)| field.value != *base);
        unsaved_guard.set(editing || form_dirty);

        let hitbox = draw_form_interactive(
            stdout,
            title,
            fields,
            selected,
            hovered,
            footer_hover,
            editing,
            &cursors,
            &revealed_secrets,
        )?;
        let key = match read_event()? {
            ConfigEvent::Resize => continue,
            ConfigEvent::Paste(text) => {
                apply_form_paste(fields, selected, editing, &mut cursors, &text);
                continue;
            }
            ConfigEvent::Key(key) => key,
            ConfigEvent::Mouse(mouse) => {
                // 1. 【配置表单】【悬停追踪】同步追踪字段行、底部保存取消按钮及底栏快捷段
                if matches!(mouse.kind, MouseEventKind::Moved) {
                    let next = form_hover_target(fields, &hitbox, mouse.column, mouse.row);
                    let next_footer = hitbox.footer.rect_at(mouse.column, mouse.row);
                    if next != hovered || next_footer != footer_hover {
                        hovered = next;
                        footer_hover = next_footer;
                        continue;
                    }
                    match wait_form_non_hover_key(
                        stdout,
                        fields,
                        &mut selected,
                        &mut hovered,
                        &mut footer_hover,
                        &mut editing,
                        &mut fcitx,
                        &mut clicks,
                        &mut cursors,
                        &hitbox,
                    )? {
                        FormEventStep::Key(k) => k,
                        FormEventStep::Paste(text) => {
                            apply_form_paste(fields, selected, editing, &mut cursors, &text);
                            continue;
                        }
                        FormEventStep::Redraw => continue,
                        FormEventStep::Finish(saved) => return Ok(saved),
                    }
                } else {
                    match apply_form_mouse(
                        stdout,
                        mouse,
                        fields,
                        &hitbox,
                        &mut selected,
                        &mut editing,
                        &mut fcitx,
                        &mut clicks,
                        &mut cursors,
                    )? {
                        FormEventStep::Redraw => continue,
                        FormEventStep::Paste(text) => {
                            apply_form_paste(fields, selected, editing, &mut cursors, &text);
                            continue;
                        }
                        FormEventStep::Finish(saved) => return Ok(saved),
                        FormEventStep::Key(key) => key,
                    }
                }
            }
        };
        match key.code {
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                fcitx.leave_editing();
                return Ok(true);
            }
            // 2. 【配置表单】【Tab顺滑切换】支持 Tab 与 Shift+Tab 在字段及保存/取消按钮间循环跳转
            KeyCode::Tab => {
                if editing {
                    fcitx.leave_editing();
                    editing = false;
                }
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    selected = tab_previous(fields, selected);
                } else {
                    selected = tab_next(fields, selected);
                }
            }
            KeyCode::BackTab => {
                if editing {
                    fcitx.leave_editing();
                    editing = false;
                }
                selected = tab_previous(fields, selected);
            }
            KeyCode::Esc if editing => {
                fcitx.leave_editing();
                editing = false;
            }
            KeyCode::Esc | KeyCode::Char('q') if !editing => return Ok(false),
            KeyCode::Enter if editing => {
                fcitx.leave_editing();
                editing = false;
            }
            KeyCode::Enter if !editing && selected == fields.len() => return Ok(true),
            KeyCode::Enter if !editing && selected == fields.len() + 1 => return Ok(false),
            // 3. 【配置表单】【只读保护】只读字段禁止任何编辑或轮换操作
            _ if !editing && selected < fields.len() && fields[selected].readonly => {
                match key.code {
                    KeyCode::Char('s') => return Ok(true),
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = previous_selectable(fields, selected)
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = next_selectable(fields, selected)
                    }
                    _ => {}
                }
            }
            KeyCode::Enter
            | KeyCode::Char(' ')
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Char('h' | 'l')
                if !editing && selected < fields.len() && fields[selected].boolean =>
            {
                let value = parse_bool_field(&fields[selected].value)?;
                fields[selected].value = (!value).to_string();
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l')
                if !editing && selected < fields.len() && !fields[selected].choices.is_empty() =>
            {
                cycle_choice(&mut fields[selected], true);
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Left | KeyCode::Char('h')
                if !editing && selected < fields.len() && !fields[selected].choices.is_empty() =>
            {
                cycle_choice(&mut fields[selected], false);
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter
                if !editing && selected < fields.len() && !fields[selected].choices.is_empty() =>
            {
                if fields[selected].choices.len() <= INLINE_CHOICE_LIMIT {
                    cycle_choice(&mut fields[selected], true);
                } else {
                    fields[selected].value = select_choice(
                        stdout,
                        &fields[selected].label,
                        &fields[selected].value,
                        &fields[selected].choices,
                        fields[selected].empty_choice_label,
                    )?;
                }
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter if !editing && selected < fields.len() && fields[selected].textarea => {
                edit_textarea(stdout, &mut fields[selected].value)?;
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter if !editing && selected < fields.len() => {
                if !fields[selected].boolean && !fields[selected].readonly {
                    fcitx.enter_editing();
                    editing = true;
                }
            }
            KeyCode::Char('s') if !editing => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') if !editing => {
                selected = previous_selectable(fields, selected)
            }
            KeyCode::Down | KeyCode::Char('j') if !editing => {
                selected = next_selectable(fields, selected)
            }
            KeyCode::Left | KeyCode::Char('h') if !editing && selected == fields.len() + 1 => {
                selected = fields.len()
            }
            KeyCode::Right | KeyCode::Char('l') if !editing && selected == fields.len() => {
                selected = fields.len() + 1
            }
            KeyCode::Left if editing => cursors[selected] = cursors[selected].saturating_sub(1),
            KeyCode::Right if editing => {
                cursors[selected] =
                    (cursors[selected] + 1).min(fields[selected].value.chars().count())
            }
            KeyCode::Home if editing => cursors[selected] = 0,
            KeyCode::End if editing => cursors[selected] = fields[selected].value.chars().count(),
            KeyCode::Backspace if editing => {
                if cursors[selected] > 0 {
                    remove_char_before_cursor(&mut fields[selected].value, &mut cursors[selected]);
                }
            }
            KeyCode::Delete if editing => {
                remove_char_at_cursor(&mut fields[selected].value, cursors[selected])
            }
            KeyCode::Char('r')
                if editing
                    && fields[selected].secret
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                revealed_secrets[selected] = !revealed_secrets[selected];
            }
            KeyCode::Char(char) if editing && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                insert_char_at_cursor(&mut fields[selected].value, &mut cursors[selected], char)
            }
            _ => {}
        }
    }
}

/// 【配置表单】【粘贴写入】在单行文本编辑态下将净化后的粘贴文本写入光标处。
fn apply_form_paste(
    fields: &mut [Field],
    selected: usize,
    editing: bool,
    cursors: &mut [usize],
    text: &str,
) {
    if !editing || selected >= fields.len() {
        return;
    }
    let field = &mut fields[selected];
    if field.readonly || field.boolean || field.textarea || !field.choices.is_empty() {
        return;
    }
    let cleaned = sanitize_paste_single_line(text);
    if !cleaned.is_empty() {
        super::search::insert_str_at_char(&mut field.value, &mut cursors[selected], &cleaned);
    }
}

/// 【配置表单】【行内轮换】在枚举选项间前后切换，跳过额外的全屏选择弹窗。
pub(super) fn cycle_choice(field: &mut Field, forward: bool) {
    if field.choices.is_empty() || field.readonly {
        return;
    }
    let current = field
        .choices
        .iter()
        .position(|item| item == &field.value)
        .unwrap_or(0);
    let len = field.choices.len();
    let next = if forward {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    };
    field.value = field.choices[next].clone();
}

fn first_selectable(fields: &[Field]) -> usize {
    fields.iter().position(|field| !field.section).unwrap_or(0)
}

pub(super) fn next_selectable(fields: &[Field], selected: usize) -> usize {
    let mut next = selected.saturating_add(1);
    while next < fields.len() && fields[next].section {
        next += 1;
    }
    next.min(fields.len() + 1)
}

pub(super) fn previous_selectable(fields: &[Field], selected: usize) -> usize {
    if selected == 0 {
        return first_selectable(fields);
    }
    let mut next = selected - 1;
    loop {
        if next >= fields.len() || !fields[next].section {
            return next;
        }
        if next == 0 {
            return first_selectable(fields);
        }
        next -= 1;
    }
}

/// 【配置表单】【Tab正向循环】按 Tab 依次经过所有可选字段、保存与取消按钮并在末尾折返首项。
fn tab_next(fields: &[Field], selected: usize) -> usize {
    if selected >= fields.len() + 1 {
        first_selectable(fields)
    } else {
        next_selectable(fields, selected)
    }
}

/// 【配置表单】【BackTab反向循环】按 Shift+Tab 逆向切换焦点，在首项处折返至底部按钮。
fn tab_previous(fields: &[Field], selected: usize) -> usize {
    let first = first_selectable(fields);
    if selected <= first {
        fields.len() + 1
    } else {
        previous_selectable(fields, selected)
    }
}

fn insert_char_at_cursor(value: &mut String, cursor: &mut usize, ch: char) {
    let byte_index = byte_index_for_char(value, *cursor);
    value.insert(byte_index, ch);
    *cursor += 1;
}

fn remove_char_before_cursor(value: &mut String, cursor: &mut usize) {
    let end = byte_index_for_char(value, *cursor);
    let start = byte_index_for_char(value, cursor.saturating_sub(1));
    value.replace_range(start..end, "");
    *cursor -= 1;
}

fn remove_char_at_cursor(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index_for_char(value, cursor);
    let end = byte_index_for_char(value, cursor + 1);
    value.replace_range(start..end, "");
}

fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

pub(super) fn take_chars(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn empty_vision_provider_stays_empty() {
        let config = AppConfig::default();
        assert!(vision_provider_value(&config).is_empty());
    }

    #[test]
    fn navigation_skips_section_rows() {
        let fields = vec![
            Field::section("组一"),
            Field::new("A", String::new()),
            Field::section("组二"),
            Field::new("B", String::new()),
        ];

        assert_eq!(first_selectable(&fields), 1);
        assert_eq!(next_selectable(&fields, 1), 3);
        assert_eq!(previous_selectable(&fields, 3), 1);
        assert_eq!(previous_selectable(&fields, 1), 1);
        assert_eq!(next_selectable(&fields, 3), 4);
    }

    #[test]
    fn tab_and_backtab_cycle_through_fields_and_action_buttons() {
        let fields = vec![
            Field::section("分组"),
            Field::new("A", String::new()),
            Field::new("B", String::new()),
        ];
        assert_eq!(tab_next(&fields, 1), 2);
        assert_eq!(tab_next(&fields, 2), 3);
        assert_eq!(tab_next(&fields, 3), 4);
        assert_eq!(tab_next(&fields, 4), 1);

        assert_eq!(tab_previous(&fields, 1), 4);
        assert_eq!(tab_previous(&fields, 4), 3);
        assert_eq!(tab_previous(&fields, 3), 2);
    }

    #[test]
    fn choice_field_cycles_forward_and_backward_inline() {
        let mut field =
            Field::new("Mode", "audited".into()).choices(&["yolo", "audited", "auto_audit"]);
        cycle_choice(&mut field, true);
        assert_eq!(field.value, "auto_audit");
        cycle_choice(&mut field, true);
        assert_eq!(field.value, "yolo");
        cycle_choice(&mut field, false);
        assert_eq!(field.value, "auto_audit");
    }
}
