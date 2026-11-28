//! 群组工作台事件分发：统一处理键盘快捷键、多行输入、历史浏览、斜杠与成员补全、
//! 鼠标悬停/点击/滚轮分区及 `/model` 交互切换触发。

use super::modal_input::handle_modal_key;
use super::state::{GroupModal, GroupTuiState, InspectorTab};
use crate::cli::repl_input::shortcuts::{
    apply_composer_edit_key, is_composer_newline_key, ComposerEditOutcome,
};
use crate::cli::repl_input_navigation::{
    move_cursor_down_by_visual_row, move_cursor_up_by_visual_row,
};
use crate::cli::repl_text::{
    insert_str_at_cursor, is_disallowed_control_char, strip_terminal_control_sequences,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// 终端群组输入事件解析后的上层业务动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GroupAction {
    None,
    Exit,
    Redraw,
    Reload,
    SelectModel,
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
        admission: String,
    },
    GatewayControl {
        action: String,
        target: String,
    },
    RunWatchdog,
}

/// 【群组终端】【事件分发】统一处理键盘快捷键、多行编辑、自动补全、粘贴、弹窗与鼠标交互。
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
        Event::Paste(text) => handle_paste(state, &text),
        Event::FocusLost => {
            state.pointer = None;
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 处理括号粘贴事件：主输入框保留多行换行，单行表单弹窗清洗换行为空格。
fn handle_paste(state: &mut GroupTuiState, text: &str) -> GroupAction {
    let sanitized = strip_terminal_control_sequences(text)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if let Some(modal) = state.modal.as_mut() {
        let clean = sanitized.replace('\n', " ");
        if let Some((buf, cursor)) = modal.active_field_mut() {
            *cursor = (*cursor).min(buf.chars().count());
            insert_str_at_cursor(buf, cursor, &clean);
        }
    } else {
        state.insert_str(&sanitized);
    }
    GroupAction::None
}

/// 处理主界面键盘事件。
fn handle_key(state: &mut GroupTuiState, key: KeyEvent) -> GroupAction {
    if state.modal.is_some() {
        return handle_modal_key(state, key);
    }

    // 1. 补全面板激活时优先响应上下选择、Tab/Enter 补全与 Esc 静默收起
    let completions = state.completion_items();
    if !completions.is_empty() {
        match key.code {
            KeyCode::Up if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                let len = completions.len() as isize;
                state.completion_idx =
                    (state.completion_idx as isize - 1).rem_euclid(len) as usize;
                return GroupAction::None;
            }
            KeyCode::Down if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                state.completion_idx = (state.completion_idx + 1) % completions.len();
                return GroupAction::None;
            }
            KeyCode::Tab => {
                state.apply_active_completion();
                return GroupAction::None;
            }
            KeyCode::Enter
                if !key
                    .modifiers
                    .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL) =>
            {
                let chosen = &completions[state.completion_idx.min(completions.len() - 1)];
                let is_partial_slash = state.input.starts_with('/')
                    && !state.input.contains(char::is_whitespace)
                    && !state.input.eq_ignore_ascii_case(&chosen.label);
                if is_partial_slash || state.input.ends_with('@') {
                    state.apply_active_completion();
                    return GroupAction::None;
                }
            }
            KeyCode::Esc => {
                state.completions_dismissed = true;
                state.completion_idx = 0;
                // 终端将快速连按两次 Esc 合并为 Alt+Esc 时同步清空当前草稿
                if key.modifiers.contains(KeyModifiers::ALT) && !state.input.is_empty() {
                    state.input.clear();
                    state.cursor = 0;
                    state.completions_dismissed = false;
                }
                return GroupAction::None;
            }
            _ => {}
        }
    }

    // 2. 换行快捷键（Shift+Enter / Alt+Enter / Ctrl+J，与主 REPL 完全一致）
    if is_composer_newline_key(key.code, key.modifiers) {
        state.insert_newline();
        return GroupAction::None;
    }

    // 3. Ctrl 组合快捷键（群组操作与切群）
    if key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('q' | 'Q' | 'c' | 'C') => return GroupAction::Exit,
            KeyCode::Char('l' | 'L') => return GroupAction::Redraw,
            KeyCode::Char('n' | 'N') => {
                state.clear_transient_notice();
                state.modal = Some(GroupModal::CreateGroup {
                    name: String::new(),
                    description: String::new(),
                    field: 0,
                    cursor: 0,
                });
                return GroupAction::None;
            }
            KeyCode::Char('r' | 'R') => {
                state.clear_transient_notice();
                let default_name = next_runner_name(state);
                let cursor = default_name.chars().count();
                state.modal = Some(GroupModal::SpawnRunner {
                    name: default_name,
                    specialty: "general".to_string(),
                    prompt: String::new(),
                    field: 0,
                    cursor,
                });
                return GroupAction::None;
            }
            KeyCode::Char('g' | 'G') => {
                state.clear_transient_notice();
                state.modal = Some(GroupModal::BindChannel {
                    channel: "weixin".to_string(),
                    chat_id: "*".to_string(),
                    admission: "auto".to_string(),
                    field: 0,
                    cursor: "weixin".chars().count(),
                });
                return GroupAction::None;
            }
            KeyCode::Char('k' | 'K') => {
                state.clear_transient_notice();
                state.modal = Some(GroupModal::ChannelSim {
                    channel: "weixin".to_string(),
                    chat_id: "wx_demo".to_string(),
                    actor: "ChannelUser".to_string(),
                    content: String::new(),
                    field: 3,
                    cursor: 0,
                });
                return GroupAction::None;
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
                state.set_notice("请先在右侧「总览」成员列表中选中 Runner，再按 Ctrl+X 停止");
                return GroupAction::None;
            }
            KeyCode::Up => {
                return if state.step_group(-1) {
                    GroupAction::Reload
                } else {
                    GroupAction::None
                };
            }
            KeyCode::Down => {
                return if state.step_group(1) {
                    GroupAction::Reload
                } else {
                    GroupAction::None
                };
            }
            _ => {}
        }
    }

    // 4. Alt 组合键：切成员与直达右侧检视标签
    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Up => {
                state.step_member(-1);
                return GroupAction::None;
            }
            KeyCode::Down => {
                state.step_member(1);
                return GroupAction::None;
            }
            KeyCode::Char('1') => {
                state.inspector_open = true;
                state.inspector_tab = InspectorTab::Overview;
                state.inspector_scroll = 0;
                return GroupAction::None;
            }
            KeyCode::Char('2') => {
                state.inspector_open = true;
                state.inspector_tab = InspectorTab::ChannelsGateways;
                state.inspector_scroll = 0;
                return GroupAction::None;
            }
            KeyCode::Char('3') => {
                state.inspector_open = true;
                state.inspector_tab = InspectorTab::ToolsDesign;
                state.inspector_scroll = 0;
                return GroupAction::None;
            }
            KeyCode::Char('4') => {
                state.inspector_open = true;
                state.inspector_tab = InspectorTab::ThreadsWatchdog;
                state.inspector_scroll = 0;
                return GroupAction::None;
            }
            _ => {}
        }
    }

    // 5. 空输入时按 End 回到最新消息，否则交给共享输入编辑键处理行尾跳转
    if key.code == KeyCode::End && state.input.is_empty() {
        state.chat_scroll = 0;
        state.unseen_chat = false;
        return GroupAction::None;
    }

    // 6. 复用 Sai 共享输入框编辑快捷键（Home/End/Ctrl+A/E/W/U/K、Ctrl/Alt+←→，全为字符级偏移）
    state.clamp_cursor();
    if let Some(outcome) =
        apply_composer_edit_key(&mut state.input, &mut state.cursor, key.code, key.modifiers)
    {
        if outcome == ComposerEditOutcome::TextEdited {
            state.on_input_mutated();
        }
        return GroupAction::None;
    }

    match key.code {
        KeyCode::Esc => {
            if !state.input.is_empty() {
                state.input.clear();
                state.cursor = 0;
                state.completion_idx = 0;
                state.completions_dismissed = false;
                state.set_notice("已清空输入；再按一次 Esc 退出群组工作台");
                GroupAction::None
            } else {
                GroupAction::Exit
            }
        }
        KeyCode::F(1) => {
            state.clear_transient_notice();
            state.modal = Some(GroupModal::Help);
            GroupAction::None
        }
        KeyCode::F(2) => {
            state.clear_transient_notice();
            state.inspector_open = !state.inspector_open;
            GroupAction::None
        }
        KeyCode::Tab => {
            state.clear_transient_notice();
            state.inspector_open = true;
            state.inspector_tab = state.inspector_tab.next();
            state.inspector_scroll = 0;
            GroupAction::None
        }
        KeyCode::F(5) => GroupAction::RunWatchdog,
        KeyCode::PageUp => {
            state.chat_scroll = state.chat_scroll.saturating_add(10);
            GroupAction::None
        }
        KeyCode::PageDown => {
            state.chat_scroll = state.chat_scroll.saturating_sub(10);
            if state.chat_scroll == 0 {
                state.unseen_chat = false;
            }
            GroupAction::None
        }
        KeyCode::Up => {
            let cols = state.hit_map.composer_content_cols.max(16);
            if !state.input.is_empty() {
                if let Some(prev) = move_cursor_up_by_visual_row("", &state.input, state.cursor, cols)
                {
                    state.cursor = prev;
                    return GroupAction::None;
                }
            }
            if !state.history_up() && state.input.is_empty() {
                state.chat_scroll = state.chat_scroll.saturating_add(2);
            }
            GroupAction::None
        }
        KeyCode::Down => {
            let cols = state.hit_map.composer_content_cols.max(16);
            if !state.input.is_empty() {
                if let Some(next) =
                    move_cursor_down_by_visual_row("", &state.input, state.cursor, cols)
                {
                    state.cursor = next;
                    return GroupAction::None;
                }
            }
            if !state.history_down() && state.input.is_empty() {
                state.chat_scroll = state.chat_scroll.saturating_sub(2);
                if state.chat_scroll == 0 {
                    state.unseen_chat = false;
                }
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
        KeyCode::Delete => {
            state.delete_forward();
            GroupAction::None
        }
        KeyCode::Enter => submit_composer(state),
        KeyCode::Char('?')
            if state.input.is_empty()
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            state.clear_transient_notice();
            state.modal = Some(GroupModal::Help);
            GroupAction::None
        }
        KeyCode::Char(ch)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                && !is_disallowed_control_char(ch) =>
        {
            state.insert_char(ch);
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 解析并提交底部原生输入框内容（支持普通多行对话、`@成员` 及全部群组斜杠命令）。
fn submit_composer(state: &mut GroupTuiState) -> GroupAction {
    let raw = std::mem::take(&mut state.input);
    state.cursor = 0;
    state.completion_idx = 0;
    state.completions_dismissed = false;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return GroupAction::None;
    }
    state.push_history(trimmed);

    if trimmed.eq_ignore_ascii_case("/model") || trimmed.eq_ignore_ascii_case("/models") {
        return GroupAction::SelectModel;
    }
    if trimmed.eq_ignore_ascii_case("/help") {
        state.modal = Some(GroupModal::Help);
        return GroupAction::None;
    }
    if trimmed.eq_ignore_ascii_case("/tools") {
        state.inspector_open = true;
        state.inspector_tab = InspectorTab::ToolsDesign;
        state.inspector_scroll = 0;
        state.set_notice("已切换到「工具」页查看 Leader 与 Runner 工具边界");
        return GroupAction::None;
    }
    if trimmed.eq_ignore_ascii_case("/channels") {
        state.inspector_open = true;
        state.inspector_tab = InspectorTab::ChannelsGateways;
        state.inspector_scroll = 0;
        state.set_notice("已切换到「渠道」页查看微信 / QQ / 飞书网关状态");
        return GroupAction::None;
    }
    if let Some(rest) = trimmed.strip_prefix("/gateway") {
        let mut parts = rest.trim().split_whitespace();
        let action = parts.next().unwrap_or("status").to_ascii_lowercase();
        let target = parts.next().unwrap_or("weixin").to_ascii_lowercase();
        return GroupAction::GatewayControl { action, target };
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
        state.set_error("当前没有选中群组，请先按 Ctrl+N 新建群组");
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
        state.set_error("用法：/spawn <runner名> <任务说明>");
        return GroupAction::None;
    }

    if let Some(rest) = trimmed.strip_prefix("/bind ") {
        let mut parts = rest.trim().splitn(3, char::is_whitespace);
        let channel = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        let chat_id = parts.next().unwrap_or("*").trim();
        let admission = parts.next().unwrap_or("auto").trim();
        if !channel.is_empty() {
            return GroupAction::BindChannel {
                group_id,
                channel,
                chat_id: if chat_id.is_empty() {
                    "*".to_string()
                } else {
                    chat_id.to_string()
                },
                admission: if admission.is_empty() {
                    "auto".to_string()
                } else {
                    admission.to_string()
                },
            };
        }
        state.set_error("用法：/bind <weixin|feishu|qq|onebot> [chat_id|*] [auto|explicit_only]");
        return GroupAction::None;
    }

    if let Some(rest) = trimmed
        .strip_prefix("/channel ")
        .or_else(|| trimmed.strip_prefix("/sim "))
    {
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
        state.set_error("用法：/sim <weixin|feishu|qq|onebot> <chat_id> <消息内容>");
        return GroupAction::None;
    }

    GroupAction::SendChat {
        group_id,
        content: trimmed.to_string(),
        recipient_id: None,
    }
}

/// 处理鼠标悬停、点击（侧栏开关、回到底部、模型切换、补全项、群组、成员、标签）与分区滚轮滚动。
fn handle_mouse(state: &mut GroupTuiState, mouse: MouseEvent) -> GroupAction {
    if state.modal.is_some() {
        return GroupAction::None;
    }
    if mouse.kind == MouseEventKind::Moved {
        state.pointer = Some((mouse.column, mouse.row));
        return GroupAction::None;
    }

    let over_sidebar =
        state.inspector_open && mouse.column >= state.hit_map.sidebar_start_col;
    let over_completion = !state.hit_map.completion_hits.is_empty()
        && mouse.column < state.hit_map.completion_end_col
        && mouse.row >= state.hit_map.composer_row_range.0;
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if over_completion {
                let len = state.completion_items().len();
                if len > 0 {
                    state.completion_idx =
                        (state.completion_idx as isize - 1).rem_euclid(len as isize) as usize;
                }
            } else if over_sidebar {
                state.inspector_scroll = state.inspector_scroll.saturating_sub(2);
            } else {
                state.chat_scroll = state.chat_scroll.saturating_add(3);
            }
            GroupAction::None
        }
        MouseEventKind::ScrollDown => {
            if over_completion {
                let len = state.completion_items().len();
                if len > 0 {
                    state.completion_idx = (state.completion_idx + 1) % len;
                }
            } else if over_sidebar {
                state.inspector_scroll = state.inspector_scroll.saturating_add(2);
            } else {
                state.chat_scroll = state.chat_scroll.saturating_sub(3);
                if state.chat_scroll == 0 {
                    state.unseen_chat = false;
                }
            }
            GroupAction::None
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // 1. 顶栏右上角侧栏开关 `◫`
            if mouse.row == 0 {
                if let Some((start, end)) = state.hit_map.header_sidebar_toggle {
                    if mouse.column >= start && mouse.column < end {
                        state.inspector_open = !state.inspector_open;
                        return GroupAction::None;
                    }
                }
            }

            // 2. 聊天区底部“回到底部”悬浮胶囊
            if let Some((row, start, end)) = state.hit_map.bottom_button {
                if mouse.row == row && mouse.column >= start && mouse.column < end {
                    state.chat_scroll = 0;
                    state.unseen_chat = false;
                    return GroupAction::None;
                }
            }

            // 3. 底栏模型/思考信息（点击打开交互式模型选择器）
            if let Some((row, start, end)) = state.hit_map.footer_model_hit {
                if mouse.row == row && mouse.column >= start && mouse.column < end {
                    return GroupAction::SelectModel;
                }
            }

            // 4. 输入区下方补全候选行点击
            if mouse.column < state.hit_map.completion_end_col {
                if let Some(&(_, comp_idx)) = state
                    .hit_map
                    .completion_hits
                    .iter()
                    .find(|(row, _)| *row == mouse.row)
                {
                    let items = state.completion_items();
                    let ready_command = items
                        .get(comp_idx)
                        .is_some_and(|item| item.insert_text.starts_with('/') && !item.insert_text.ends_with(' '));
                    if state.apply_completion_at(comp_idx) && ready_command {
                        return submit_composer(state);
                    }
                    return GroupAction::None;
                }
            }

            // 5. 右侧栏模型摘要行、标签页、群组列表与成员列表点击
            if over_sidebar {
                if state.hit_map.sidebar_model_row == Some(mouse.row)
                    && mouse.column > state.hit_map.sidebar_start_col
                {
                    state.clear_transient_notice();
                    return GroupAction::SelectModel;
                }
                if let Some(&(_, _, _, tab)) = state
                    .hit_map
                    .tab_hits
                    .iter()
                    .find(|(row, start, end, _)| {
                        *row == mouse.row && mouse.column >= *start && mouse.column < *end
                    })
                {
                    state.clear_transient_notice();
                    state.inspector_tab = tab;
                    state.inspector_scroll = 0;
                    return GroupAction::None;
                }
                if let Some(&(_, idx)) = state
                    .hit_map
                    .group_rows
                    .iter()
                    .find(|(row, _)| *row == mouse.row)
                {
                    if idx != state.selected_group_idx {
                        state.selected_group_idx = idx;
                        state.chat_scroll = 0;
                        state.unseen_chat = false;
                        state.inspector_scroll = 0;
                        state.selected_member_idx = 0;
                        state.leader_live = Default::default();
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
            GroupAction::None
        }
        _ => GroupAction::None,
    }
}

/// 生成下一个不重名的默认 Runner 名称。
fn next_runner_name(state: &GroupTuiState) -> String {
    let existing = state
        .detail
        .as_ref()
        .map(|d| d.members.len())
        .unwrap_or(1);
    format!("runner-{existing}")
}
