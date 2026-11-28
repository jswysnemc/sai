//! 群组弹窗键盘处理：统一字符级光标编辑，复用主输入区共享词级/行级编辑快捷键。

use super::input::GroupAction;
use super::state::{GroupModal, GroupTuiState};
use crate::cli::repl_input::shortcuts::apply_composer_edit_key;
use crate::cli::repl_text::{
    insert_char_at_cursor, is_disallowed_control_char, remove_char_at_cursor,
    remove_char_before_cursor,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 【群组终端】【弹窗键盘处理】统一处理圆角弹窗内的光标移动、词/行编辑、字段切换与表单提交。
///
/// 参数:
/// - `state`: 群组 TUI 状态
/// - `key`: 键盘事件
///
/// 返回:
/// - 提交产生的上层业务动作或 `GroupAction::None`
pub(super) fn handle_modal_key(state: &mut GroupTuiState, key: KeyEvent) -> GroupAction {
    if matches!(state.modal, Some(GroupModal::Help)) {
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'Q' | '?') | KeyCode::F(1)
        ) {
            state.modal = None;
        }
        return GroupAction::None;
    }

    // 1. 优先复用共享词级/行级编辑快捷键（Home/End/Ctrl+A/E/W/U/K、Ctrl/Alt+←→）
    if let Some(modal) = state.modal.as_mut() {
        if let Some((buf, cursor)) = modal.active_field_mut() {
            *cursor = (*cursor).min(buf.chars().count());
            if apply_composer_edit_key(buf, cursor, key.code, key.modifiers).is_some() {
                return GroupAction::None;
            }
        }
    }

    // 2. 处理字段切换、字符增删与表单提交
    match key.code {
        KeyCode::Esc => {
            state.modal = None;
            GroupAction::None
        }
        KeyCode::Tab | KeyCode::Down => {
            if let Some(modal) = state.modal.as_mut() {
                modal.step_field(1);
            }
            GroupAction::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            if let Some(modal) = state.modal.as_mut() {
                modal.step_field(-1);
            }
            GroupAction::None
        }
        KeyCode::Left => {
            if let Some(modal) = state.modal.as_mut() {
                if let Some((buf, cursor)) = modal.active_field_mut() {
                    *cursor = (*cursor).min(buf.chars().count()).saturating_sub(1);
                }
            }
            GroupAction::None
        }
        KeyCode::Right => {
            if let Some(modal) = state.modal.as_mut() {
                if let Some((buf, cursor)) = modal.active_field_mut() {
                    let len = buf.chars().count();
                    *cursor = ((*cursor).min(len) + 1).min(len);
                }
            }
            GroupAction::None
        }
        KeyCode::Backspace => {
            if let Some(modal) = state.modal.as_mut() {
                if let Some((buf, cursor)) = modal.active_field_mut() {
                    *cursor = (*cursor).min(buf.chars().count());
                    if *cursor > 0 {
                        remove_char_before_cursor(buf, cursor);
                    }
                }
            }
            GroupAction::None
        }
        KeyCode::Delete => {
            if let Some(modal) = state.modal.as_mut() {
                if let Some((buf, cursor)) = modal.active_field_mut() {
                    let pos = (*cursor).min(buf.chars().count());
                    *cursor = pos;
                    remove_char_at_cursor(buf, pos);
                }
            }
            GroupAction::None
        }
        KeyCode::Enter => submit_modal(state),
        KeyCode::Char(ch)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && !is_disallowed_control_char(ch) =>
        {
            if let Some(modal) = state.modal.as_mut() {
                if let Some((buf, cursor)) = modal.active_field_mut() {
                    *cursor = (*cursor).min(buf.chars().count());
                    insert_char_at_cursor(buf, cursor, ch);
                }
            }
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 校验并提交当前弹窗表单，转换为对应业务动作。
fn submit_modal(state: &mut GroupTuiState) -> GroupAction {
    let Some(modal) = state.modal.take() else {
        return GroupAction::None;
    };
    match modal {
        GroupModal::CreateGroup {
            name, description, ..
        } => {
            if name.trim().is_empty() {
                state.set_error("群组名称不能为空");
                return GroupAction::None;
            }
            GroupAction::CreateGroup {
                name: name.trim().to_string(),
                description: description.trim().to_string(),
            }
        }
        GroupModal::SpawnRunner {
            name,
            specialty,
            prompt,
            ..
        } => {
            let Some(group_id) = state.selected_group_id().map(ToOwned::to_owned) else {
                return GroupAction::None;
            };
            if name.trim().is_empty() || prompt.trim().is_empty() {
                state.set_error("Runner 名称和任务指令均不能为空");
                return GroupAction::None;
            }
            GroupAction::SpawnRunner {
                group_id,
                name: name.trim().to_string(),
                specialty: specialty.trim().to_string(),
                prompt: prompt.trim().to_string(),
            }
        }
        GroupModal::BindChannel {
            channel,
            chat_id,
            admission,
            ..
        } => {
            let Some(group_id) = state.selected_group_id().map(ToOwned::to_owned) else {
                return GroupAction::None;
            };
            let ch = channel.trim().to_ascii_lowercase();
            if ch.is_empty() {
                state.set_error("渠道类型不能为空（如 weixin / feishu / qq / onebot）");
                return GroupAction::None;
            }
            GroupAction::BindChannel {
                group_id,
                channel: ch,
                chat_id: if chat_id.trim().is_empty() {
                    "*".to_string()
                } else {
                    chat_id.trim().to_string()
                },
                admission: if admission.trim().is_empty() {
                    "auto".to_string()
                } else {
                    admission.trim().to_string()
                },
            }
        }
        GroupModal::ChannelSim {
            channel,
            chat_id,
            actor,
            content,
            ..
        } => {
            let Some(group_id) = state.selected_group_id().map(ToOwned::to_owned) else {
                return GroupAction::None;
            };
            if content.trim().is_empty() {
                state.set_error("模拟消息内容不能为空");
                return GroupAction::None;
            }
            GroupAction::ChannelInbound {
                group_id,
                channel: if channel.trim().is_empty() {
                    "weixin".to_string()
                } else {
                    channel.trim().to_string()
                },
                chat_id: if chat_id.trim().is_empty() {
                    "wx_demo".to_string()
                } else {
                    chat_id.trim().to_string()
                },
                actor: if actor.trim().is_empty() {
                    "ChannelUser".to_string()
                } else {
                    actor.trim().to_string()
                },
                content: content.trim().to_string(),
            }
        }
        GroupModal::Help => GroupAction::None,
    }
}
