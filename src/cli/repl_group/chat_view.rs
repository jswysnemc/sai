//! 群聊正文画布：对齐 Sai 主会话 transcript 视觉风格（2 列视觉引导、无重背景胶囊、轻量化竖线结构卡、
//! 原生单列圆环实时活动条与“回到底部”悬浮胶囊）。

use super::model_info::short_member_model;
use super::state::GroupTuiState;
use super::text_util::{
    activity_spinner, clip_ansi, format_hhmm, pad_ansi, scrollbar_column, wrap_plain, BOLD, RESET,
};
use crate::cli::repl_text::visible_width;
use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::panel_palette::{bg, fg, PanelPalette, Rgb};
use crate::render::render_width::with_render_width;
use crate::render::transcript::AnsiLine;
use crate::super_agent::model::{
    GroupMessage, MemberRole, MemberStatus, MessageKind, MessageSenderKind,
};

/// 【群组终端】【群聊流渲染】以 Sai 原生 transcript 风格渲染多角色群聊时间线、实时活动条与“回到底部”胶囊。
///
/// 参数:
/// - `state`: 群组 TUI 状态（同步更新总行数、视口高度与悬浮按钮热区）
/// - `width`: 左侧主画布总列宽
/// - `height`: 群聊正文区可用行数
/// - `body_top_row`: 群聊正文区在屏幕中的起始行号
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 长度等于 `height`、每行宽 `width` 的渲染行数组
pub(super) fn render_chat_pane(
    state: &mut GroupTuiState,
    width: usize,
    height: usize,
    body_top_row: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if height == 0 {
        return Vec::new();
    }
    let Some(detail) = &state.detail else {
        let mut out = vec![pad_ansi(
            &format!(
                "  {}尚未选择群组，按 Ctrl+N 新建协作群组。{RESET}",
                fg(palette.muted)
            ),
            width,
        )];
        out.resize(height, " ".repeat(width));
        return out;
    };

    // 1. 预留底部实时思考 / Runner 活动状态条高度
    let live_bar = build_live_activity_bar(state, width.saturating_sub(3), palette);
    let live_rows = live_bar.len().min(height.saturating_sub(1));
    let viewport_h = height.saturating_sub(live_rows).max(1);
    let text_col_w = width.saturating_sub(1).max(16);
    let content_w = text_col_w.saturating_sub(3).max(14);

    // 2. 渲染顶部 Sai 原生品牌与模型摘要头 + 全量群聊消息（对齐主 TUI 视觉引导列，取消四边边框与实心胶囊）
    let mut rendered: Vec<String> = Vec::new();
    append_group_brand_header(&mut rendered, &detail.group.name, state, content_w + 3, palette);
    if detail.messages.is_empty() {
        append_empty_welcome(&mut rendered, palette);
    } else {
        rendered.push(String::new());
        for msg in &detail.messages {
            rendered.extend(format_message_block(msg, state, content_w, palette));
            rendered.push(String::new());
        }
    }

    let total = rendered.len();
    state.chat_total_lines = total;
    state.chat_viewport_rows = viewport_h;

    let max_scroll = total.saturating_sub(viewport_h);
    let scroll = state.chat_scroll.min(max_scroll);
    state.chat_scroll = scroll;
    if scroll == 0 {
        state.unseen_chat = false;
    }
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(viewport_h);
    let slice = &rendered[start..end];
    let sb = scrollbar_column(viewport_h, total, start, palette);

    let mut out = Vec::with_capacity(height);
    for row in 0..viewport_h {
        let mut body = pad_ansi(slice.get(row).map(String::as_str).unwrap_or(""), text_col_w);
        // 3. 离开底部时在正文末行叠加 Sai 同款“回到底部 / 有新输出”悬浮胶囊
        if row + 1 == viewport_h && scroll > 0 {
            let screen_row = (body_top_row + row) as u16;
            let (overlaid, hit) =
                overlay_bottom_capsule(&body, text_col_w, screen_row, state, palette);
            body = overlaid;
            state.hit_map.bottom_button = hit;
        }
        let bar_glyph = sb.get(row).map(String::as_str).unwrap_or(" ");
        out.push(format!("{body}{bar_glyph}"));
    }
    for row in live_bar.into_iter().take(live_rows) {
        out.push(pad_ansi(&row, width));
    }
    while out.len() < height {
        out.push(" ".repeat(width));
    }
    out
}

/// 构造对话流顶部的 Sai 原生三行品牌与模型信息头（对齐 `welcome_brand.rs`，随对话自然上滚）。
fn append_group_brand_header(
    lines: &mut Vec<String>,
    group_name: &str,
    state: &GroupTuiState,
    width: usize,
    palette: &PanelPalette,
) {
    let leader = &state.model_info.leader;
    let worker = &state.model_info.worker;
    let win = crate::cli::repl_chrome::format_token_k(leader.context_window_tokens);
    let show_logo = width >= 46;
    let accent = fg(palette.accent);
    let text_fg = fg(palette.text);
    let muted = fg(palette.muted);
    let subtle = fg(palette.subtle);

    // 1. 组装三行信息列：标题 / 模型配置 / 工作区目录（窄屏也完整保留模型基本信息）
    let info_budget = if show_logo {
        width.saturating_sub(10).max(16)
    } else {
        width.saturating_sub(3).max(16)
    };
    let title_line = clip_ansi(
        &format!(
            "{text_fg}{BOLD}sai{RESET}  {muted}group{RESET} {subtle}·{RESET} {accent}{group_name}{RESET}"
        ),
        info_budget,
    );
    let model_value = if state.model_info.worker_shares_leader {
        format!(
            "{text_fg}{}{RESET} {muted}({}) · {win} · 并发 {}{RESET}",
            leader.short_model, leader.thinking, state.model_info.max_concurrent_workers
        )
    } else {
        format!(
            "{text_fg}{}{RESET} {muted}({}) · worker {}{RESET}",
            leader.short_model, leader.thinking, worker.short_model
        )
    };
    let model_line = clip_ansi(&format!("{muted}model{RESET} {model_value}"), info_budget);
    let dir_line = clip_ansi(
        &format!(
            "{muted}dir{RESET}   {muted}{}{RESET}",
            state.model_info.directory
        ),
        info_budget,
    );
    let info_rows = [title_line, model_line, dir_line];

    // 2. 宽屏左侧拼装 Sai 同形单笔画圆角 S 标志（2 列左缩进与主全屏画布严格对齐）
    let logo = [
        format!("  {accent}{BOLD}╭──╴{RESET} {accent}●{RESET}  "),
        format!("  {accent}{BOLD}╰──╮{RESET}    "),
        format!("  {accent}{BOLD}╶──╯{RESET}    "),
    ];
    for (idx, info) in info_rows.into_iter().enumerate() {
        if show_logo {
            lines.push(format!("{}{info}", logo[idx]));
        } else {
            lines.push(format!("  {info}"));
        }
    }
}

/// 构造无消息时的轻量操作引导。
fn append_empty_welcome(lines: &mut Vec<String>, palette: &PanelPalette) {
    lines.push(String::new());
    for tip in [
        "直接发送消息交由 Leader 拆解任务并异步调度 Runner",
        "输入 @ 提及群组成员，输入 / 使用快捷命令（含 /model 切换模型）",
        "按 Tab 切换右侧「渠道 / 工具 / 守护」视图，按 ? 查看快捷键",
    ] {
        lines.push(format!(
            "   {}· {}{tip}{RESET}",
            fg(palette.subtle),
            fg(palette.muted)
        ));
    }
}

/// 构造群聊底部的实时思考 / 工具调用 / Runner 运行状态条（使用 Sai 原生单列圆环 `◜ ◝ ◞ ◟`）。
fn build_live_activity_bar(
    state: &GroupTuiState,
    avail: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let mut rows = Vec::new();
    let spin = activity_spinner(state.tick);
    if state.leader_live.thinking {
        let activity = if state.leader_live.activity.trim().is_empty() {
            "正在思考与编排..."
        } else {
            state.leader_live.activity.as_str()
        };
        let model_tag = format!(
            "{} · {}",
            state.model_info.leader.short_model, state.model_info.leader.thinking
        );
        let line = format!(
            " {spin} {}{BOLD}Leader{RESET} {}{activity}{RESET} {}· {model_tag}{RESET}",
            fg(palette.accent),
            fg(palette.text),
            fg(palette.subtle),
        );
        rows.push(clip_ansi(&line, avail));
        if !state.leader_live.draft_preview.trim().is_empty() {
            let draft = format!(
                "   {}│ {}{}",
                fg(palette.border),
                fg(palette.muted),
                state
                    .leader_live
                    .draft_preview
                    .replace(['\n', '\u{3000}'], " ")
            );
            rows.push(format!("{}{RESET}", clip_ansi(&draft, avail)));
        }
    }
    if let Some(detail) = &state.detail {
        let running: Vec<String> = detail
            .members
            .iter()
            .filter(|m| m.role == MemberRole::Worker && m.status == MemberStatus::Running)
            .map(|m| {
                let clean_act = m
                    .last_activity
                    .replace(['\n', '\u{3000}'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if clean_act.is_empty() {
                    m.name.clone()
                } else {
                    format!("{} ({})", m.name, clip_ansi(&clean_act, 30))
                }
            })
            .collect();
        if !running.is_empty() {
            let line = format!(
                " {spin} {}Runner{RESET} {}{}",
                fg(palette.added),
                fg(palette.muted),
                running.join(" · ")
            );
            rows.push(format!("{}{RESET}", clip_ansi(&line, avail)));
        }
    }
    rows
}

/// 将单条群组消息渲染为对齐 Sai 主会话风格的简洁时间线块。
fn format_message_block(
    msg: &GroupMessage,
    state: &GroupTuiState,
    width: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let ts = format_hhmm(msg.created_at);
    let target = msg
        .recipient_name
        .as_deref()
        .map(|name| format!(" {}→ @{name}{RESET}", fg(palette.muted)))
        .unwrap_or_default();
    let channel = msg
        .channel_source
        .as_deref()
        .filter(|ch| !matches!(*ch, "" | "group" | "tui"))
        .map(|ch| format!(" {}via {ch}{RESET}", fg(palette.accent)))
        .unwrap_or_default();

    // 1. 用户普通消息：对齐主全屏 `user_echo_cell.rs`（高对比橙点 `●` + 加粗正文，零沉重胶囊）
    if msg.sender_kind == MessageSenderKind::User && msg.kind == MessageKind::Chat {
        let mut lines = Vec::new();
        let body_lines = AnsiLine::wrap_block(msg.content.trim_end(), width);
        let meta_suffix = format!("{target}{channel} {}{ts}{RESET}", fg(palette.subtle));
        if body_lines.len() == 1 && visible_width(body_lines[0].as_str()) + 12 <= width {
            lines.push(format!(
                " \x1b[1;38;5;208m●{RESET} {}{BOLD}{}{RESET} {meta_suffix}",
                fg(palette.text),
                body_lines[0].as_str()
            ));
        } else {
            lines.push(format!(
                " \x1b[1;38;5;208m●{RESET} {}{BOLD}你{RESET}{meta_suffix}",
                fg(palette.text)
            ));
            for line in body_lines {
                lines.push(format!("   {}{BOLD}{}{RESET}", fg(palette.text), line.as_str()));
            }
        }
        return lines;
    }

    // 2. 系统普通通知：对齐主 REPL `›` 静默提示行，应用中文避头标点折行并将时间戳自然续排到文末
    if msg.sender_kind == MessageSenderKind::System && msg.kind != MessageKind::Alert {
        let mut wrapped = Vec::new();
        for raw_line in msg.content.trim_end().lines() {
            wrapped.extend(wrap_plain(raw_line, width));
        }
        if wrapped.is_empty() {
            wrapped.push(String::new());
        }
        let ts_w = visible_width(&ts) + 2;
        let last_fits = wrapped
            .last()
            .is_some_and(|line| visible_width(line) + ts_w <= width);
        let mut lines = Vec::with_capacity(wrapped.len() + usize::from(!last_fits));
        let last_idx = wrapped.len() - 1;
        for (idx, line) in wrapped.into_iter().enumerate() {
            let prefix = if idx == 0 {
                format!(" {}› {RESET}", fg(palette.subtle))
            } else {
                "   ".to_string()
            };
            if idx == last_idx && last_fits {
                lines.push(format!(
                    "{prefix}{}{line}{RESET}  {}{ts}{RESET}",
                    fg(palette.muted),
                    fg(palette.subtle)
                ));
            } else {
                lines.push(format!("{prefix}{}{line}{RESET}", fg(palette.muted)));
            }
        }
        if !last_fits {
            lines.push(format!("   {}{ts}{RESET}", fg(palette.subtle)));
        }
        return lines;
    }

    // 3. 角色与结构化消息（Leader / Runner / 成果交付 / 执行报告 / 任务派工 / 告警）
    let (glyph, role_label, color, use_guide) = classify_message_style(msg, palette);
    let sender_extra = match msg.sender_kind {
        MessageSenderKind::Worker => {
            if role_label == "Runner" {
                format!(" {}{BOLD}{}{RESET}", fg(palette.text), msg.sender_name)
            } else {
                format!(" {}· {}{RESET}", fg(palette.muted), msg.sender_name)
            }
        }
        MessageSenderKind::Leader if role_label != "Leader" => {
            format!(" {}· {}{RESET}", fg(palette.muted), msg.sender_name)
        }
        MessageSenderKind::User => {
            if msg.sender_name.eq_ignore_ascii_case("you") || msg.sender_name == "你" {
                String::new()
            } else {
                format!(" {}· {}{RESET}", fg(palette.muted), msg.sender_name)
            }
        }
        _ => String::new(),
    };
    let model_hint = resolve_message_model_hint(msg, state)
        .map(|model| format!(" {}· {model}{RESET}", fg(palette.subtle)))
        .unwrap_or_default();

    let header = if use_guide {
        format!(
            " {}{glyph} {BOLD}{role_label}{RESET}{sender_extra}{target}{channel}{model_hint} {}· {ts}{RESET}",
            fg(color),
            fg(palette.subtle),
        )
    } else {
        let title_str = if msg.sender_kind == MessageSenderKind::Worker {
            format!(
                "{}{BOLD}{}{RESET} {}· Runner{RESET}",
                fg(palette.text),
                msg.sender_name,
                fg(palette.muted)
            )
        } else {
            format!("{}{BOLD}{role_label}{RESET}", fg(palette.text))
        };
        format!(
            " {}{glyph}{RESET} {title_str}{target}{channel}{model_hint} {}· {ts}{RESET}",
            fg(color),
            fg(palette.subtle),
        )
    };

    let body_w = if use_guide {
        width.saturating_sub(2).max(12)
    } else {
        width.max(12)
    };
    let body_lines = render_markdown_lines(&msg.content, body_w);
    let mut lines = Vec::with_capacity(body_lines.len() + 1);
    lines.push(header);
    for line in body_lines {
        if use_guide {
            lines.push(format!("   {}│{RESET} {line}", fg(palette.border)));
        } else {
            lines.push(format!("   {line}"));
        }
    }
    lines
}

/// 查询消息发送者对应的紧凑模型名，仅在 Leader / Worker 发言时轻量补充。
fn resolve_message_model_hint(msg: &GroupMessage, state: &GroupTuiState) -> Option<String> {
    match msg.sender_kind {
        MessageSenderKind::Leader => {
            let short = &state.model_info.leader.short_model;
            (!short.is_empty() && short != "-").then(|| short.clone())
        }
        MessageSenderKind::Worker => {
            let member_model = state
                .detail
                .as_ref()
                .and_then(|detail| detail.members.iter().find(|m| m.id == msg.sender_id))
                .map(|m| short_member_model(&m.model_label))
                .filter(|s| !s.is_empty());
            member_model.or_else(|| {
                let short = &state.model_info.worker.short_model;
                (!short.is_empty() && short != "-").then(|| short.clone())
            })
        }
        _ => None,
    }
}

/// 按消息类型和发送方确定引导字形、角色标签、语义色与是否使用轻量化竖线引导。
fn classify_message_style(
    msg: &GroupMessage,
    palette: &PanelPalette,
) -> (&'static str, &'static str, Rgb, bool) {
    match (&msg.sender_kind, &msg.kind) {
        (_, MessageKind::Delivery) => ("✓", "成果交付", palette.added, true),
        (_, MessageKind::Report) => ("●", "执行报告", palette.warning, true),
        (_, MessageKind::Dispatch) => ("◆", "任务派工", palette.accent, true),
        (_, MessageKind::ChannelIn) => ("◆", "渠道入站", palette.accent, false),
        (_, MessageKind::Alert) => ("×", "系统告警", palette.removed, true),
        (MessageSenderKind::Watchdog, _) => ("◆", "守护巡检", palette.removed, true),
        (MessageSenderKind::Leader, _) => ("●", "Leader", palette.accent, false),
        (MessageSenderKind::Worker, _) => ("●", "Runner", palette.added, false),
        (MessageSenderKind::User, _) => ("●", "你", palette.text, false),
        (MessageSenderKind::System, _) => ("›", "系统", palette.subtle, false),
    }
}

/// 复用 Sai 内置 Markdown 渲染器将消息正文渲染并安全折行到目标宽度。
fn render_markdown_lines(content: &str, width: usize) -> Vec<String> {
    let rendered = with_render_width(width, || {
        let mut md = MarkdownStreamRenderer::new_stable();
        let mut out = md.push(content);
        out.push_str(&md.flush());
        out
    });
    let trimmed = rendered.trim_end_matches('\n');
    if trimmed.is_empty() {
        return vec![String::new()];
    }
    AnsiLine::wrap_block(trimmed, width)
        .into_iter()
        .map(|line| line.as_str().to_string())
        .collect()
}

/// 在正文末行水平居中叠加 Sai 同款“回到底部 / 有新输出”悬浮胶囊（对齐 `fullscreen/bottom_button.rs`）。
fn overlay_bottom_capsule(
    line: &str,
    width: usize,
    screen_row: u16,
    state: &GroupTuiState,
    palette: &PanelPalette,
) -> (String, Option<(u16, u16, u16)>) {
    let text = if state.unseen_chat {
        " ↓ 有新输出 "
    } else {
        " ↓ 回到底部 "
    };
    let btn_w = visible_width(text);
    if btn_w + 4 > width {
        return (line.to_string(), None);
    }
    let start = (width - btn_w) / 2;
    let end = start + btn_w;
    let hovered = state.pointer.is_some_and(|(col, row)| {
        row == screen_row && usize::from(col) >= start && usize::from(col) < end
    });
    let pill_bg = bg(if hovered {
        palette.focus_bg
    } else {
        palette.hover_bg
    });
    let pill_fg = fg(if hovered || state.unseen_chat {
        palette.accent
    } else {
        palette.text
    });
    let capsule = format!(" {pill_bg}{pill_fg}{text}{RESET} ");
    let overlay_start = start.saturating_sub(1);
    let overlay_w = btn_w + 2;
    let merged = crate::render::ansi_overlay::overlay_columns(line, overlay_start, overlay_w, &capsule);
    (
        pad_ansi(&merged, width),
        Some((screen_row, start as u16, end as u16)),
    )
}
