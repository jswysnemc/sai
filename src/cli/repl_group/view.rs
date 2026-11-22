use super::state::{GroupModal, GroupTuiState, InspectorTab};
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::render::panel_palette::{bg, fg, PanelPalette};
use crate::super_agent::model::{
    GroupDetailSnapshot, GroupMessage, MemberRole, MemberStatus, MessageKind, MessageSenderKind,
    TaskStatus, WatchdogSeverity,
};

const RESET: &str = "\x1b[0m";

/// 【群组终端】【整屏渲染】按终端尺寸响应式绘制顶栏、左侧群组与成员栏、中间群聊流、右侧检视栏与底部输入框。
///
/// 参数:
/// - `state`: 群组 TUI 状态（同步更新本帧点击热区）
/// - `size`: 终端 `(cols, rows)`
///
/// 返回:
/// - 完整屏幕行数组（长度等于 `rows`）
pub(super) fn render(state: &mut GroupTuiState, size: (u16, u16)) -> Vec<String> {
    let cols = usize::from(size.0).max(40);
    let rows = usize::from(size.1).max(12);
    let palette = PanelPalette::current();
    state.hit_map = Default::default();

    let show_inspector = state.inspector_open && cols >= 104;
    let left_w = if cols >= 96 {
        26usize
    } else if cols >= 72 {
        20usize
    } else {
        16usize
    };
    let right_w = if show_inspector {
        (cols / 4).clamp(28, 38)
    } else {
        0usize
    };
    let center_w = cols
        .saturating_sub(left_w + 1)
        .saturating_sub(if show_inspector { right_w + 1 } else { 0 })
        .max(20);

    // 顶栏 1 行 + 分隔线 1 行 + 主体 body_rows + 输入框 2 行 + 底栏 1 行
    let body_rows = rows.saturating_sub(5).max(4);
    let body_start_row = 2usize;

    state.hit_map.left_end_col = left_w as u16;
    state.hit_map.chat_col_range = ((left_w + 1) as u16, (left_w + 1 + center_w) as u16);
    state.hit_map.chat_row_range = (body_start_row as u16, (body_start_row + body_rows) as u16);

    let mut out = Vec::with_capacity(rows);
    out.push(render_header(state, cols, &palette));
    out.push(format!("{}{}{RESET}", fg(palette.border), "─".repeat(cols)));

    let left_lines = render_left_pane(state, left_w, body_rows, body_start_row, &palette);
    let center_lines = render_chat_pane(state, center_w, body_rows, &palette);
    let right_lines = if show_inspector {
        render_inspector_pane(
            state,
            right_w,
            body_rows,
            body_start_row,
            left_w + 1 + center_w + 1,
            &palette,
        )
    } else {
        Vec::new()
    };

    let sep = format!("{}│{RESET}", fg(palette.border));
    for i in 0..body_rows {
        let left = pad_ansi(
            left_lines.get(i).map(String::as_str).unwrap_or(""),
            left_w,
        );
        let center = pad_ansi(
            center_lines.get(i).map(String::as_str).unwrap_or(""),
            center_w,
        );
        if show_inspector {
            let right = pad_ansi(
                right_lines.get(i).map(String::as_str).unwrap_or(""),
                right_w,
            );
            out.push(format!("{left}{sep}{center}{sep}{right}"));
        } else {
            out.push(format!("{left}{sep}{center}"));
        }
    }

    // 输入分隔线 + 输入框 + 底部快捷键栏
    out.push(format!("{}{}{RESET}", fg(palette.border), "─".repeat(cols)));
    out.push(render_composer_line(state, cols, &palette));
    out.push(render_footer(state, cols, &palette));

    while out.len() < rows {
        out.push(" ".repeat(cols));
    }
    out.truncate(rows);

    if let Some(modal) = &state.modal {
        overlay_modal(&mut out, modal, cols, rows, &palette);
    }
    out
}

/// 渲染顶部状态栏。
fn render_header(state: &GroupTuiState, cols: usize, palette: &PanelPalette) -> String {
    let title = state
        .detail
        .as_ref()
        .map(|d| d.group.name.as_str())
        .unwrap_or("No Group");
    let (leader_badge, runner_stats, channel_count, alert_count) = match &state.detail {
        Some(detail) => {
            let leader = detail
                .members
                .iter()
                .find(|m| m.role == MemberRole::Leader)
                .map(|m| m.status.as_str())
                .unwrap_or("idle");
            let total_w = detail
                .members
                .iter()
                .filter(|m| m.role == MemberRole::Worker)
                .count();
            let running_w = detail
                .members
                .iter()
                .filter(|m| m.role == MemberRole::Worker && m.status == MemberStatus::Running)
                .count();
            let alerts = state
                .watchdog_events
                .iter()
                .filter(|e| !e.resolved && e.severity == WatchdogSeverity::ActionRequired)
                .count();
            (
                leader,
                format!("{running_w}/{total_w}"),
                detail.channels.len(),
                alerts,
            )
        }
        None => ("idle", "0/0".to_string(), 0, 0),
    };
    let left = format!(
        " {}◆ SUPER-AGENT GROUP{RESET}  \x1b[1m{}{RESET}  {}Leader:{leader_badge}  Runners:{runner_stats}  Channels:{channel_count}{RESET}",
        fg(palette.accent),
        title,
        fg(palette.muted)
    );
    let right = if alert_count > 0 {
        format!(
            "{}Watchdog:{alert_count} alert(s){RESET} ",
            fg(palette.removed)
        )
    } else {
        format!("{}Watchdog:OK{RESET} ", fg(palette.added))
    };
    join_left_right(&left, &right, cols, &bg(palette.focus_bg))
}

/// 渲染左侧「群组列表 + Leader/Runners 成员列表」栏。
fn render_left_pane(
    state: &mut GroupTuiState,
    width: usize,
    height: usize,
    body_start_row: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(height);
    lines.push(format!(
        " \x1b[1m{}GROUPS ({}){RESET}",
        fg(palette.accent),
        state.groups.len()
    ));

    let group_slots = (height / 3).clamp(3, 8);
    for (idx, summary) in state.groups.iter().take(group_slots).enumerate() {
        let row_screen = (body_start_row + lines.len()) as u16;
        state.hit_map.group_rows.push((row_screen, idx));
        let selected = idx == state.selected_group_idx;
        let prefix = if selected { "▸" } else { " " };
        let badge = format!("{}r", summary.running_worker_count);
        let text = format!(" {prefix} {} [{}]", summary.group.name, badge);
        if selected {
            lines.push(format!(
                "{}{}{}{RESET}",
                bg(palette.focus_bg),
                fg(palette.text),
                pad_plain(&text, width)
            ));
        } else {
            lines.push(format!("{}{}{RESET}", fg(palette.muted), clip_to_width(&text, width)));
        }
    }

    lines.push(format!("{}{}{RESET}", fg(palette.border), "─".repeat(width)));
    let member_count = state.detail.as_ref().map(|d| d.members.len()).unwrap_or(0);
    lines.push(format!(
        " \x1b[1m{}ROSTER ({member_count}){RESET}",
        fg(palette.accent)
    ));

    if let Some(detail) = &state.detail {
        for (idx, member) in detail.members.iter().enumerate() {
            if lines.len() >= height {
                break;
            }
            let row_screen = (body_start_row + lines.len()) as u16;
            state.hit_map.member_rows.push((row_screen, idx));
            let selected = idx == state.selected_member_idx;
            let (glyph, color) = match (member.role, member.status) {
                (MemberRole::Leader, MemberStatus::Running) => ("◆", palette.accent),
                (MemberRole::Leader, _) => ("◆", palette.added),
                (MemberRole::Worker, MemberStatus::Running) => ("●", palette.accent),
                (MemberRole::Worker, MemberStatus::Waiting) => ("◐", palette.warning),
                (MemberRole::Worker, MemberStatus::Failed) => ("×", palette.removed),
                (MemberRole::Worker, MemberStatus::Stopped) => ("○", palette.subtle),
                _ => ("○", palette.muted),
            };
            let role_tag = if member.role == MemberRole::Leader {
                "L"
            } else {
                "R"
            };
            let label = format!(" {glyph} [{role_tag}] {}", member.name);
            if selected {
                lines.push(format!(
                    "{}{}{}{RESET}",
                    bg(palette.hover_bg),
                    fg(color),
                    pad_plain(&label, width)
                ));
            } else {
                lines.push(format!(
                    "{}{}{RESET}",
                    fg(color),
                    clip_to_width(&label, width)
                ));
            }
        }
    }
    lines
}

/// 渲染中间实时群聊时间线（用户、Leader、Runners、报告、主动交付、渠道消息与 Watchdog）。
fn render_chat_pane(
    state: &GroupTuiState,
    width: usize,
    height: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let Some(detail) = &state.detail else {
        return vec![format!(
            " {}尚未选择群组，按 Ctrl+N 创建新群组。{RESET}",
            fg(palette.muted)
        )];
    };

    let mut rendered: Vec<String> = Vec::new();
    let inner_w = width.saturating_sub(2).max(12);
    for msg in &detail.messages {
        rendered.extend(format_message_lines(msg, inner_w, palette));
        rendered.push(String::new());
    }
    if rendered.is_empty() {
        rendered.push(format!(
            " {}群组对话为空。直接输入任务给 Leader，或使用 @Runner名 指定成员。{RESET}",
            fg(palette.muted)
        ));
    }

    let total = rendered.len();
    let max_scroll = total.saturating_sub(height);
    let scroll = state.chat_scroll.min(max_scroll);
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(height);
    rendered[start..end].to_vec()
}

/// 将单条群组消息格式化为带角色色标与自动换行的终端文本行。
fn format_message_lines(
    msg: &GroupMessage,
    width: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let (badge, color) = match (&msg.sender_kind, &msg.kind) {
        (_, MessageKind::Delivery) => ("[Delivery]", palette.added),
        (_, MessageKind::Report) => ("[Report]", palette.warning),
        (_, MessageKind::ChannelIn) => ("[Channel]", palette.accent),
        (_, MessageKind::Dispatch) => ("[Dispatch]", palette.accent),
        (MessageSenderKind::Watchdog, _) => ("[Watchdog]", palette.removed),
        (MessageSenderKind::Leader, _) => ("[Leader]", palette.accent),
        (MessageSenderKind::Worker, _) => ("[Runner]", palette.added),
        (MessageSenderKind::User, _) => ("[You]", palette.text),
        (MessageSenderKind::System, _) => ("[System]", palette.subtle),
    };
    let target_suffix = msg
        .recipient_name
        .as_deref()
        .map(|name| format!(" -> @{name}"))
        .unwrap_or_default();
    let channel_suffix = msg
        .channel_source
        .as_deref()
        .filter(|ch| !matches!(*ch, "" | "group" | "tui"))
        .map(|ch| format!(" via {ch}"))
        .unwrap_or_default();
    let header = format!(
        " {}{badge} {}{target_suffix}{channel_suffix}{RESET}",
        fg(color),
        msg.sender_name
    );

    let mut lines = vec![header];
    let body_color = if msg.kind == MessageKind::Delivery {
        fg(palette.text)
    } else if msg.sender_kind == MessageSenderKind::System {
        fg(palette.muted)
    } else {
        fg(palette.text)
    };
    for raw_line in msg.content.lines() {
        for wrapped in wrap_text(raw_line, width.saturating_sub(2).max(10)) {
            lines.push(format!("   {body_color}{wrapped}{RESET}"));
        }
    }
    lines
}

/// 渲染右侧检视面板（Tasks & Reports / Threads & Triggers / Channels & Watchdog）。
fn render_inspector_pane(
    state: &mut GroupTuiState,
    width: usize,
    height: usize,
    body_start_row: usize,
    pane_start_col: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let mut lines = Vec::with_capacity(height);
    // 1. 顶部标签页切换条
    let tabs = [
        InspectorTab::TasksReports,
        InspectorTab::ThreadsTriggers,
        InspectorTab::ChannelsWatchdog,
    ];
    let mut tab_bar = String::from(" ");
    let mut cursor_col = pane_start_col + 1;
    for tab in tabs {
        let label = tab.label();
        let start = cursor_col as u16;
        let end = (cursor_col + label.len() + 2) as u16;
        state
            .hit_map
            .tab_hits
            .push((body_start_row as u16, start, end, tab));
        if tab == state.inspector_tab {
            tab_bar.push_str(&format!(
                "{}{}[{label}]{RESET} ",
                bg(palette.focus_bg),
                fg(palette.accent)
            ));
        } else {
            tab_bar.push_str(&format!("{} {label} {RESET}", fg(palette.muted)));
        }
        cursor_col += label.len() + 3;
    }
    lines.push(tab_bar);
    lines.push(format!("{}{}{RESET}", fg(palette.border), "─".repeat(width)));

    let Some(detail) = &state.detail else {
        return lines;
    };
    match state.inspector_tab {
        InspectorTab::TasksReports => render_tasks_reports_tab(detail, width, palette, &mut lines),
        InspectorTab::ThreadsTriggers => {
            render_threads_triggers_tab(detail, width, palette, &mut lines)
        }
        InspectorTab::ChannelsWatchdog => {
            render_channels_watchdog_tab(detail, &state.watchdog_events, width, palette, &mut lines)
        }
    }
    lines
}

/// 渲染「任务与 Runner 报告」检视页。
fn render_tasks_reports_tab(
    detail: &GroupDetailSnapshot,
    width: usize,
    palette: &PanelPalette,
    lines: &mut Vec<String>,
) {
    lines.push(format!(
        " \x1b[1m{}TASKS ({}){RESET}",
        fg(palette.accent),
        detail.tasks.len()
    ));
    if detail.tasks.is_empty() {
        lines.push(format!("  {}暂无任务{RESET}", fg(palette.muted)));
    } else {
        for task in detail.tasks.iter().take(6) {
            let (glyph, color) = match task.status {
                TaskStatus::Delivered => ("✓", palette.added),
                TaskStatus::Running => ("●", palette.accent),
                TaskStatus::WaitingReview => ("◆", palette.warning),
                TaskStatus::Failed | TaskStatus::Blocked => ("×", palette.removed),
                _ => ("○", palette.muted),
            };
            let runner = task.assigned_member_name.as_deref().unwrap_or("-");
            let text = format!("  {glyph} {} [{runner}]", task.title);
            lines.push(format!(
                "{}{}{RESET}",
                fg(color),
                clip_to_width(&text, width)
            ));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        " \x1b[1m{}REPORTS ({}){RESET}",
        fg(palette.warning),
        detail.reports.len()
    ));
    if detail.reports.is_empty() {
        lines.push(format!("  {}暂无 Runner 报告{RESET}", fg(palette.muted)));
    } else {
        for report in detail.reports.iter().take(5) {
            let tag = if report.synthesized_by_leader {
                "✓"
            } else {
                "!"
            };
            let text = format!(
                "  {tag} {}: {}",
                report.worker_name,
                report.summary.replace('\n', " ")
            );
            lines.push(format!(
                "{}{}{RESET}",
                fg(palette.text),
                clip_to_width(&text, width)
            ));
        }
    }
}

/// 渲染「线程订阅与两级过滤 Trigger」检视页。
fn render_threads_triggers_tab(
    detail: &GroupDetailSnapshot,
    width: usize,
    palette: &PanelPalette,
    lines: &mut Vec<String>,
) {
    lines.push(format!(
        " \x1b[1m{}SUBSCRIPTIONS ({}){RESET}",
        fg(palette.accent),
        detail.subscriptions.len()
    ));
    for sub in detail.subscriptions.iter().take(6) {
        let member_name = detail
            .members
            .iter()
            .find(|m| m.id == sub.member_id)
            .map(|m| m.name.as_str())
            .unwrap_or(&sub.member_id);
        let text = format!("  ● {member_name} ({}) -> {}", sub.mode.as_str(), sub.thread_id);
        lines.push(format!(
            "{}{}{RESET}",
            fg(palette.text),
            clip_to_width(&text, width)
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        " \x1b[1m{}TRIGGERS ({}){RESET}",
        fg(palette.warning),
        detail.triggers.len()
    ));
    for trig in detail.triggers.iter().take(6) {
        let text = format!(
            "  [{}] {}: {}",
            trig.route_path,
            trig.source_channel,
            trig.content.replace('\n', " ")
        );
        lines.push(format!(
            "{}{}{RESET}",
            fg(palette.muted),
            clip_to_width(&text, width)
        ));
    }
}

/// 渲染「渠道绑定、交付回执与 Watchdog」检视页。
fn render_channels_watchdog_tab(
    detail: &GroupDetailSnapshot,
    watchdog_events: &[crate::super_agent::model::WatchdogEvent],
    width: usize,
    palette: &PanelPalette,
    lines: &mut Vec<String>,
) {
    lines.push(format!(
        " \x1b[1m{}CHANNELS ({}){RESET}",
        fg(palette.accent),
        detail.channels.len()
    ));
    if detail.channels.is_empty() {
        lines.push(format!(
            "  {}未绑定外部渠道（/bind 绑定）{RESET}",
            fg(palette.muted)
        ));
    } else {
        for ch in &detail.channels {
            let text = format!("  ● {}:{} [{}]", ch.channel, ch.external_chat_id, ch.admission_mode);
            lines.push(format!(
                "{}{}{RESET}",
                fg(palette.added),
                clip_to_width(&text, width)
            ));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        " \x1b[1m{}DELIVERIES ({}){RESET}",
        fg(palette.added),
        detail.deliveries.len()
    ));
    for d in detail.deliveries.iter().take(4) {
        let text = format!("  ✓ {} [{}]: {}", d.channel, d.status, d.preview);
        lines.push(format!(
            "{}{}{RESET}",
            fg(palette.text),
            clip_to_width(&text, width)
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        " \x1b[1m{}WATCHDOG ({}){RESET}",
        fg(palette.warning),
        watchdog_events.len()
    ));
    for ev in watchdog_events.iter().take(4) {
        let color = match ev.severity {
            WatchdogSeverity::Healthy | WatchdogSeverity::Recovered => palette.added,
            WatchdogSeverity::Notice => palette.warning,
            WatchdogSeverity::ActionRequired => palette.removed,
        };
        let text = format!("  [{}] {}", ev.severity.as_str(), ev.summary);
        lines.push(format!(
            "{}{}{RESET}",
            fg(color),
            clip_to_width(&text, width)
        ));
    }
}

/// 渲染底部输入框。
fn render_composer_line(state: &GroupTuiState, cols: usize, palette: &PanelPalette) -> String {
    let prompt = format!(" {}›{RESET} ", fg(palette.accent));
    let avail = cols.saturating_sub(4).max(10);
    if state.input.is_empty() {
        let placeholder = "发消息给 Leader，或 @Runner 沟通，或 /spawn <名> <任务> | /channel <渠道> <会话> <内容>";
        return format!(
            "{prompt}{}{}{RESET}",
            fg(palette.subtle),
            clip_to_width(placeholder, avail)
        );
    }
    let display = clip_to_width(&state.input, avail.saturating_sub(1));
    format!("{prompt}{}{display}█{RESET}", fg(palette.text))
}

/// 渲染底部快捷键与状态提示栏。
fn render_footer(state: &GroupTuiState, cols: usize, palette: &PanelPalette) -> String {
    let hints = " Enter发送  Ctrl+N新建群组  Ctrl+R启动Runner  Ctrl+K模拟渠道  F2检视栏  F5巡检  Esc退出";
    let right = state
        .notice
        .as_deref()
        .map(|text| format!(" {text} "))
        .unwrap_or_default();
    let left = format!("{}{hints}{RESET}", fg(palette.muted));
    let right_styled = format!("{}{right}{RESET}", fg(palette.accent));
    join_left_right(&left, &right_styled, cols, &bg(palette.focus_bg))
}

/// 在画面中央叠加弹窗对话框。
fn overlay_modal(
    screen: &mut [String],
    modal: &GroupModal,
    cols: usize,
    rows: usize,
    palette: &PanelPalette,
) {
    let box_w = cols.saturating_sub(8).clamp(36, 68);
    let fields: Vec<(&str, &str)> = match modal {
        GroupModal::CreateGroup {
            name, description, ..
        } => vec![("Group Name", name.as_str()), ("Description", description.as_str())],
        GroupModal::SpawnRunner {
            name,
            specialty,
            prompt,
            ..
        } => vec![
            ("Runner Name", name.as_str()),
            ("Specialty", specialty.as_str()),
            ("Task Prompt", prompt.as_str()),
        ],
        GroupModal::ChannelSim {
            channel,
            chat_id,
            actor,
            content,
            ..
        } => vec![
            ("Channel (weixin/feishu/qq/onebot)", channel.as_str()),
            ("External Chat ID", chat_id.as_str()),
            ("Actor Name", actor.as_str()),
            ("Message Content", content.as_str()),
        ],
    };
    let active_field = match modal {
        GroupModal::CreateGroup { field, .. }
        | GroupModal::SpawnRunner { field, .. }
        | GroupModal::ChannelSim { field, .. } => *field,
    };

    let mut box_lines = Vec::new();
    let inner_w = box_w.saturating_sub(2);
    box_lines.push(format!(
        "{}{}┌{}┐{RESET}",
        bg(palette.focus_bg),
        fg(palette.accent),
        "─".repeat(inner_w)
    ));
    box_lines.push(format!(
        "{}{}│\x1b[1m{}{RESET}{}{}│{RESET}",
        bg(palette.focus_bg),
        fg(palette.accent),
        pad_plain(&format!(" {} ", modal.title()), inner_w),
        bg(palette.focus_bg),
        fg(palette.accent),
    ));
    box_lines.push(format!(
        "{}{}├{}┤{RESET}",
        bg(palette.focus_bg),
        fg(palette.border),
        "─".repeat(inner_w)
    ));
    for (idx, (label, value)) in fields.iter().enumerate() {
        let active = idx == active_field;
        let marker = if active { "▸" } else { " " };
        let cursor = if active { "█" } else { "" };
        let row_text = format!(" {marker} {label}: {value}{cursor}");
        let color = if active { palette.text } else { palette.muted };
        box_lines.push(format!(
            "{}{}│{}{}{}│{RESET}",
            bg(palette.focus_bg),
            fg(palette.border),
            fg(color),
            pad_plain(&row_text, inner_w),
            fg(palette.border),
        ));
    }
    box_lines.push(format!(
        "{}{}│{}{}{}│{RESET}",
        bg(palette.focus_bg),
        fg(palette.border),
        fg(palette.subtle),
        pad_plain(" Tab切换字段  Enter确认提交  Esc取消", inner_w),
        fg(palette.border),
    ));
    box_lines.push(format!(
        "{}{}└{}┘{RESET}",
        bg(palette.focus_bg),
        fg(palette.accent),
        "─".repeat(inner_w)
    ));

    let start_row = rows.saturating_sub(box_lines.len()) / 2;
    let pad_left = " ".repeat(cols.saturating_sub(box_w) / 2);
    for (offset, line) in box_lines.into_iter().enumerate() {
        if let Some(slot) = screen.get_mut(start_row + offset) {
            *slot = format!("{pad_left}{line}");
        }
    }
}

/// 按显示宽度简单折行。
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    for ch in text.chars() {
        let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if current_w + ch_w > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_w = 0;
        }
        current.push(ch);
        current_w += ch_w;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// 按显示宽度裁剪并用空格填充纯文本。
fn pad_plain(text: &str, width: usize) -> String {
    let clipped = clip_to_width(text, width);
    let w = visible_width(&clipped);
    format!("{clipped}{}", " ".repeat(width.saturating_sub(w)))
}

/// 按显示宽度填充含 ANSI 转义序列的单行文本。
fn pad_ansi(text: &str, width: usize) -> String {
    let w = visible_width(text);
    if w >= width {
        clip_to_width(text, width)
    } else {
        format!("{text}{}", " ".repeat(width - w))
    }
}

/// 左右贴边拼接单行状态栏。
fn join_left_right(left: &str, right: &str, cols: usize, background: &str) -> String {
    let lw = visible_width(left);
    let rw = visible_width(right);
    if lw + rw >= cols {
        return format!("{background}{}{RESET}", clip_to_width(left, cols));
    }
    format!(
        "{background}{left}{}{right}{RESET}",
        " ".repeat(cols - lw - rw)
    )
}
