use super::state::{GroupModal, GroupTuiState};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// 终端群组输入事件解析后的上层业务动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GroupAction {
    None,
    Exit,
    Reload,
    SendChat {
        group_id: String,
        content: String,
        recipient_id: Option<String>,
    },
    CreateGroup {
        name: String,
        description: String,
    },
    SpawnRunner {
        group_id: String,
        name: String,
        specialty: String,
        prompt: String,
    },
    StopSelectedRunner {
        group_id: String,
        member_id: String,
        member_name: String,
    },
    ChannelInbound {
        group_id: String,
        channel: String,
        chat_id: String,
        actor: String,
        content: String,
    },
    BindChannel {
        group_id: String,
        channel: String,
        chat_id: String,
    },
    RunWatchdog,
}

/// 【群组终端】【事件分发】统一处理键盘输入、快捷键、弹窗编辑与鼠标点击/滚轮。
///
/// 参数:
/// - `state`: 群组 TUI 状态
/// - `event`: 原始终端事件
///
/// 返回:
/// - 待执行的业务动作
pub(super) fn handle_event(state: &mut GroupTuiState, event: Event) -> GroupAction {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => handle_key(state, key),
        Event::Mouse(mouse) => handle_mouse(state, mouse),
        Event::Paste(text) => {
            let clean = text.replace(['\r', '\n'], " ");
            if let Some(modal) = state.modal.as_mut() {
                modal.active_text_mut().push_str(&clean);
            } else {
                for ch in clean.chars() {
                    state.insert_char(ch);
                }
            }
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 处理键盘输入。
fn handle_key(state: &mut GroupTuiState, key: KeyEvent) -> GroupAction {
    if state.modal.is_some() {
        return handle_modal_key(state, key);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('q' | 'Q' | 'c' | 'C') => GroupAction::Exit,
            KeyCode::Char('n' | 'N') => {
                state.modal = Some(GroupModal::CreateGroup {
                    name: String::new(),
                    description: String::new(),
                    field: 0,
                });
                GroupAction::None
            }
            KeyCode::Char('r' | 'R') => {
                state.modal = Some(GroupModal::SpawnRunner {
                    name: "runner-1".to_string(),
                    specialty: "general".to_string(),
                    prompt: String::new(),
                    field: 0,
                });
                GroupAction::None
            }
            KeyCode::Char('k' | 'K') => {
                state.modal = Some(GroupModal::ChannelSim {
                    channel: "weixin".to_string(),
                    chat_id: "wx_demo".to_string(),
                    actor: "ChannelUser".to_string(),
                    content: String::new(),
                    field: 3,
                });
                GroupAction::None
            }
            KeyCode::Char('x' | 'X') => {
                if let (Some(group_id), Some(worker)) =
                    (state.selected_group_id(), state.selected_worker())
                {
                    return GroupAction::StopSelectedRunner {
                        group_id: group_id.to_string(),
                        member_id: worker.id.clone(),
                        member_name: worker.name.clone(),
                    };
                }
                state.set_notice("请先在左侧 Roster 选中一个 Runner 再按 Ctrl+X 停止");
                GroupAction::None
            }
            KeyCode::Up => {
                if state.step_group(-1) {
                    GroupAction::Reload
                } else {
                    GroupAction::None
                }
            }
            KeyCode::Down => {
                if state.step_group(1) {
                    GroupAction::Reload
                } else {
                    GroupAction::None
                }
            }
            _ => GroupAction::None,
        };
    }

    match key.code {
        KeyCode::Esc => GroupAction::Exit,
        KeyCode::F(2) => {
            state.inspector_open = !state.inspector_open;
            GroupAction::None
        }
        KeyCode::Tab => {
            state.inspector_tab = state.inspector_tab.next();
            GroupAction::None
        }
        KeyCode::F(5) => GroupAction::RunWatchdog,
        KeyCode::PageUp => {
            state.chat_scroll = state.chat_scroll.saturating_add(8);
            GroupAction::None
        }
        KeyCode::PageDown => {
            state.chat_scroll = state.chat_scroll.saturating_sub(8);
            GroupAction::None
        }
        KeyCode::Up => {
            if state.input.is_empty() {
                state.chat_scroll = state.chat_scroll.saturating_add(2);
            }
            GroupAction::None
        }
        KeyCode::Down => {
            if state.input.is_empty() {
                state.chat_scroll = state.chat_scroll.saturating_sub(2);
            }
            GroupAction::None
        }
        KeyCode::Left => {
            state.move_left();
            GroupAction::None
        }
        KeyCode::Right => {
            state.move_right();
            GroupAction::None
        }
        KeyCode::Backspace => {
            state.backspace();
            GroupAction::None
        }
        KeyCode::Enter => submit_composer(state),
        KeyCode::Char(ch) => {
            state.insert_char(ch);
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 处理弹窗模式下的键盘输入。
fn handle_modal_key(state: &mut GroupTuiState, key: KeyEvent) -> GroupAction {
    match key.code {
        KeyCode::Esc => {
            state.modal = None;
            GroupAction::None
        }
        KeyCode::Tab | KeyCode::Down => {
            if let Some(modal) = state.modal.as_mut() {
                modal.next_field();
            }
            GroupAction::None
        }
        KeyCode::Backspace => {
            if let Some(modal) = state.modal.as_mut() {
                modal.active_text_mut().pop();
            }
            GroupAction::None
        }
        KeyCode::Enter => {
            let Some(modal) = state.modal.take() else {
                return GroupAction::None;
            };
            match modal {
                GroupModal::CreateGroup {
                    name, description, ..
                } => {
                    if name.trim().is_empty() {
                        state.set_notice("群组名称不能为空");
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
                        state.set_notice("Runner 名称和任务指令均不能为空");
                        return GroupAction::None;
                    }
                    GroupAction::SpawnRunner {
                        group_id,
                        name: name.trim().to_string(),
                        specialty: specialty.trim().to_string(),
                        prompt: prompt.trim().to_string(),
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
                        state.set_notice("渠道消息正文不能为空");
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
            }
        }
        KeyCode::Char(ch) => {
            if let Some(modal) = state.modal.as_mut() {
                modal.active_text_mut().push(ch);
            }
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 解析并提交输入框正文（支持普通聊天、`@成员` 以及群内快捷命令 `/new` `/spawn` `/channel` `/bind` `/watchdog`）。
fn submit_composer(state: &mut GroupTuiState) -> GroupAction {
    let raw = std::mem::take(&mut state.input);
    state.cursor = 0;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return GroupAction::None;
    }

    if let Some(rest) = trimmed.strip_prefix("/new ") {
        let name = rest.trim();
        if !name.is_empty() {
            return GroupAction::CreateGroup {
                name: name.to_string(),
                description: String::new(),
            };
        }
    }
    if trimmed.eq_ignore_ascii_case("/watchdog") {
        return GroupAction::RunWatchdog;
    }
    if trimmed.eq_ignore_ascii_case("/quit") || trimmed.eq_ignore_ascii_case("/exit") {
        return GroupAction::Exit;
    }

    let Some(group_id) = state.selected_group_id().map(ToOwned::to_owned) else {
        state.set_notice("当前没有选中群组，请先按 Ctrl+N 新建群组");
        return GroupAction::None;
    };

    if let Some(rest) = trimmed.strip_prefix("/spawn ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim();
        let prompt = parts.next().unwrap_or("").trim();
        if !name.is_empty() && !prompt.is_empty() {
            return GroupAction::SpawnRunner {
                group_id,
                name: name.to_string(),
                specialty: "general".to_string(),
                prompt: prompt.to_string(),
            };
        }
        state.set_notice("用法：/spawn <runner名> <任务说明>");
        return GroupAction::None;
    }

    if let Some(rest) = trimmed.strip_prefix("/bind ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let channel = parts.next().unwrap_or("").trim();
        let chat_id = parts.next().unwrap_or("*").trim();
        if !channel.is_empty() {
            return GroupAction::BindChannel {
                group_id,
                channel: channel.to_string(),
                chat_id: if chat_id.is_empty() {
                    "*".to_string()
                } else {
                    chat_id.to_string()
                },
            };
        }
    }

    if let Some(rest) = trimmed.strip_prefix("/channel ") {
        let mut parts = rest.trim().splitn(3, char::is_whitespace);
        let channel = parts.next().unwrap_or("").trim();
        let chat_id = parts.next().unwrap_or("").trim();
        let content = parts.next().unwrap_or("").trim();
        if !channel.is_empty() && !chat_id.is_empty() && !content.is_empty() {
            return GroupAction::ChannelInbound {
                group_id,
                channel: channel.to_string(),
                chat_id: chat_id.to_string(),
                actor: format!("{channel}_user"),
                content: content.to_string(),
            };
        }
        state.set_notice("用法：/channel <weixin|feishu|qq|onebot> <chat_id> <消息内容>");
        return GroupAction::None;
    }

    GroupAction::SendChat {
        group_id,
        content: trimmed.to_string(),
        recipient_id: None,
    }
}

/// 处理鼠标点击与群聊区滚轮滚动。
fn handle_mouse(state: &mut GroupTuiState, mouse: MouseEvent) -> GroupAction {
    if state.modal.is_some() {
        return GroupAction::None;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.chat_scroll = state.chat_scroll.saturating_add(3);
            GroupAction::None
        }
        MouseEventKind::ScrollDown => {
            state.chat_scroll = state.chat_scroll.saturating_sub(3);
            GroupAction::None
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if mouse.column < state.hit_map.left_end_col {
                if let Some(&(_, idx)) = state
                    .hit_map
                    .group_rows
                    .iter()
                    .find(|(row, _)| *row == mouse.row)
                {
                    if idx != state.selected_group_idx {
                        state.selected_group_idx = idx;
                        state.chat_scroll = 0;
                        state.selected_member_idx = 0;
                        return GroupAction::Reload;
                    }
                    return GroupAction::None;
                }
                if let Some(&(_, member_idx)) = state
                    .hit_map
                    .member_rows
                    .iter()
                    .find(|(row, _)| *row == mouse.row)
                {
                    state.selected_member_idx = member_idx;
                    if let Some(name) = state.selected_member().map(|m| m.name.clone()) {
                        state.insert_member_mention(&name);
                    }
                    return GroupAction::None;
                }
            }
            if let Some(&(_, _, _, tab)) = state
                .hit_map
                .tab_hits
                .iter()
                .find(|(row, start, end, _)| {
                    *row == mouse.row && mouse.column >= *start && mouse.column < *end
                })
            {
                state.inspector_tab = tab;
                state.inspector_open = true;
            }
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}
