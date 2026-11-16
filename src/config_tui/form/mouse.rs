//! 【配置表单】【鼠标调度】管理表单悬停、滚轮滚动、开关切换及保存/取消按钮点击。

use super::render::FormHitbox;
use super::{
    cycle_choice, edit_textarea, next_selectable, parse_bool_field, previous_selectable,
    select_choice, FcitxState, Field, INLINE_CHOICE_LIMIT,
};
use crate::config_tui::input::{read_event, ConfigEvent};
use crate::config_tui::mouse::{ClickTracker, HitRect};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::io;

/// 表单鼠标点击命中的目标区域。
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FormClick {
    Row(usize),
    Save,
    Cancel,
    Footer(KeyCode),
}

/// 鼠标与事件混合等待的内部跳转结果。
pub(super) enum FormEventStep {
    Redraw,
    Key(KeyEvent),
    Finish(bool),
}

/// 【配置表单】【悬停探测】识别指针所在位置的可交互目标，忽略分组标题行与空白区。
///
/// 参数:
/// - `fields`: 表单字段定义
/// - `hitbox`: 本帧命中区域
/// - `column`: 终端列坐标
/// - `row`: 终端行坐标
///
/// 返回:
/// - 字段下标或底部按钮虚拟下标
pub(super) fn form_hover_target(
    fields: &[Field],
    hitbox: &FormHitbox,
    column: u16,
    row: u16,
) -> Option<usize> {
    if hitbox.save.contains(column, row) {
        return Some(fields.len());
    }
    if hitbox.cancel.contains(column, row) {
        return Some(fields.len() + 1);
    }
    hitbox
        .list
        .row_at(column, row)
        .filter(|index| !fields[*index].section)
}

/// 识别表单左键单击命中目标（包含字段行、底部按钮与底栏快捷键）。
fn form_click_target(
    fields: &[Field],
    hitbox: &FormHitbox,
    column: u16,
    row: u16,
) -> Option<FormClick> {
    if hitbox.save.contains(column, row) {
        return Some(FormClick::Save);
    }
    if hitbox.cancel.contains(column, row) {
        return Some(FormClick::Cancel);
    }
    if let Some(code) = hitbox.footer.hit(column, row) {
        return Some(FormClick::Footer(code));
    }
    hitbox
        .list
        .row_at(column, row)
        .filter(|index| !fields[*index].section)
        .map(FormClick::Row)
}

/// 【配置表单】【防闪等待】鼠标同格微移时不触发重绘，直到悬停改变或触发实操作。
#[allow(clippy::too_many_arguments)]
pub(super) fn wait_form_non_hover_key(
    stdout: &mut io::Stdout,
    fields: &mut [Field],
    selected: &mut usize,
    hovered: &mut Option<usize>,
    footer_hover: &mut Option<HitRect>,
    editing: &mut bool,
    fcitx: &mut FcitxState,
    clicks: &mut ClickTracker<FormClick>,
    cursors: &mut [usize],
    hitbox: &FormHitbox,
) -> Result<FormEventStep> {
    loop {
        match read_event()? {
            ConfigEvent::Resize => return Ok(FormEventStep::Redraw),
            ConfigEvent::Key(key) => return Ok(FormEventStep::Key(key)),
            ConfigEvent::Mouse(mouse) => {
                if matches!(mouse.kind, MouseEventKind::Moved) {
                    let next = form_hover_target(fields, hitbox, mouse.column, mouse.row);
                    let next_footer = hitbox.footer.rect_at(mouse.column, mouse.row);
                    if next != *hovered || next_footer != *footer_hover {
                        *hovered = next;
                        *footer_hover = next_footer;
                        return Ok(FormEventStep::Redraw);
                    }
                    continue;
                }
                return apply_form_mouse(
                    stdout, mouse, fields, hitbox, selected, editing, fcitx, clicks, cursors,
                );
            }
        }
    }
}

/// 【配置表单】【点击与滚轮】响应字段焦点、行内开关/枚举轮换以及保存/取消按钮点击。
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_form_mouse(
    stdout: &mut io::Stdout,
    mouse: MouseEvent,
    fields: &mut [Field],
    hitbox: &FormHitbox,
    selected: &mut usize,
    editing: &mut bool,
    fcitx: &mut FcitxState,
    clicks: &mut ClickTracker<FormClick>,
    cursors: &mut [usize],
) -> Result<FormEventStep> {
    if !*editing && hitbox.frame.contains(mouse.column, mouse.row) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                *selected = previous_selectable(fields, *selected);
                clicks.clear();
                return Ok(FormEventStep::Redraw);
            }
            MouseEventKind::ScrollDown => {
                *selected = next_selectable(fields, *selected);
                clicks.clear();
                return Ok(FormEventStep::Redraw);
            }
            _ => {}
        }
    }
    let target = form_click_target(fields, hitbox, mouse.column, mouse.row);
    let Some(clicked) = clicks.update(mouse, target) else {
        return Ok(FormEventStep::Redraw);
    };
    match clicked {
        FormClick::Save => {
            fcitx.leave_editing();
            Ok(FormEventStep::Finish(true))
        }
        FormClick::Cancel => {
            fcitx.leave_editing();
            Ok(FormEventStep::Finish(false))
        }
        FormClick::Footer(code) => Ok(FormEventStep::Key(KeyEvent::new(code, KeyModifiers::NONE))),
        FormClick::Row(index) => {
            if *editing && *selected != index {
                fcitx.leave_editing();
                *editing = false;
            }
            let was_selected = *selected == index;
            *selected = index;
            if fields[index].boolean {
                let value = parse_bool_field(&fields[index].value)?;
                fields[index].value = (!value).to_string();
                cursors[index] = fields[index].value.chars().count();
            } else if !fields[index].choices.is_empty() {
                if fields[index].choices.len() <= INLINE_CHOICE_LIMIT {
                    cycle_choice(&mut fields[index], true);
                } else {
                    fields[index].value = select_choice(
                        stdout,
                        fields[index].label,
                        &fields[index].value,
                        &fields[index].choices,
                        fields[index].empty_choice_label,
                    )?;
                }
                cursors[index] = fields[index].value.chars().count();
            } else if fields[index].textarea {
                if was_selected {
                    edit_textarea(stdout, &mut fields[index].value)?;
                    cursors[index] = fields[index].value.chars().count();
                }
            } else if was_selected && !*editing {
                fcitx.enter_editing();
                *editing = true;
            }
            Ok(FormEventStep::Redraw)
        }
    }
}
