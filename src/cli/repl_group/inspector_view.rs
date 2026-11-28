//! 右侧统一侧栏四标签内容视图：总览（协作群组、含模型信息的群组成员、任务看板、Runner 上报）、
//! 渠道（本地网关与微信接入）、工具（Leader/Worker 边界）与守护（线程订阅、触发器与 Watchdog）。

use super::model_info::short_member_model;
use super::state::{GatewayStatusSummary, GroupTuiState, InspectorTab};
use super::text_util::{
    activity_spinner, badge, clean_preview_text, clip_ansi, columns, member_status_label,
    pad_ansi, section_heading, task_status_label, wrap_plain, BOLD, RESET,
};
use crate::render::panel_palette::{bg, fg, PanelPalette};
use crate::super_agent::model::{
    GroupDetailSnapshot, MemberRole, MemberStatus, TaskStatus, WatchdogEvent, WatchdogSeverity,
};
use crate::super_agent::tools::{leader_tool_design, worker_tool_design, LEADER_READONLY_TOOLS};

/// 侧栏可滚动区域中的单行条目（附带可选群组或成员点击索引）。
pub(super) struct SidebarEntry {
    pub(super) text: String,
    pub(super) group_idx: Option<usize>,
    pub(super) member_idx: Option<usize>,
}

impl SidebarEntry {
    /// 构造普通只读侧栏行。
    fn plain(text: String) -> Self {
        Self {
            text,
            group_idx: None,
            member_idx: None,
        }
    }

    /// 构造可点击切换群组的条目行。
    fn group(text: String, idx: usize) -> Self {
        Self {
            text,
            group_idx: Some(idx),
            member_idx: None,
        }
    }

    /// 构造可点击 `@提及` 成员的条目行。
    fn member(text: String, idx: usize) -> Self {
        Self {
            text,
            group_idx: None,
            member_idx: Some(idx),
        }
    }
}

/// 【群组终端】【侧栏内容分发】按当前激活标签生成全部逻辑内容行，供外层侧栏窗口裁剪与滚动。
///
/// 参数:
/// - `state`: 群组 TUI 状态
/// - `width`: 侧栏正文净宽（已扣除滚动条列）
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 带点击索引的侧栏条目列表
pub(super) fn render_tab_content(
    state: &GroupTuiState,
    width: usize,
    palette: &PanelPalette,
) -> Vec<SidebarEntry> {
    let mut entries = Vec::new();
    match state.inspector_tab {
        InspectorTab::Overview => render_overview_tab(state, width, palette, &mut entries),
        InspectorTab::ChannelsGateways => render_channels_gateways_tab(
            state.detail.as_ref(),
            &state.gateway_status,
            width,
            palette,
            &mut entries,
        ),
        InspectorTab::ToolsDesign => render_tools_design_tab(width, palette, &mut entries),
        InspectorTab::ThreadsWatchdog => {
            if let Some(detail) = &state.detail {
                render_threads_watchdog_tab(
                    detail,
                    &state.watchdog_events,
                    width,
                    palette,
                    &mut entries,
                );
            }
        }
    }
    entries
}

/// 渲染「总览」标签页：协作群组、群组成员（含实时模型与状态）、任务看板与 Runner 上报。
fn render_overview_tab(
    state: &GroupTuiState,
    width: usize,
    palette: &PanelPalette,
    out: &mut Vec<SidebarEntry>,
) {
    // 1. 协作群组分组（对齐主侧栏 `sidebar_groups_view.rs` 样式）
    let group_count = state.groups.len();
    out.push(SidebarEntry::plain(section_heading(
        "协作群组",
        &group_count.to_string(),
        group_count == 0,
        width,
        palette,
    )));
    for (idx, summary) in state.groups.iter().take(6).enumerate() {
        let selected = idx == state.selected_group_idx;
        let (glyph, tone, right_text) = if summary.running_worker_count > 0 {
            ("●", palette.accent, format!("{}r", summary.running_worker_count))
        } else if !summary.bound_channels.is_empty() {
            ("◆", palette.added, summary.bound_channels[0].clone())
        } else if selected {
            ("●", palette.accent, format!("{}m", summary.worker_count + 1))
        } else {
            ("○", palette.muted, format!("{}m", summary.worker_count + 1))
        };
        let name_style = if selected {
            format!("{}{BOLD}", fg(palette.text))
        } else {
            fg(palette.muted)
        };
        let left = format!("  {}{glyph}\x1b[39m {name_style}{}{RESET}", fg(tone), summary.group.name);
        let right = badge(&right_text, tone);
        let mut row = columns(&left, &right, width);
        if selected {
            let tint_bg = bg(palette.hover_bg);
            row = format!(
                "{tint_bg}{}{RESET}",
                pad_ansi(&row, width).replace(RESET, &format!("{RESET}{tint_bg}"))
            );
        }
        out.push(SidebarEntry::group(row, idx));
    }

    // 2. 群组成员分组（Leader + Runners，清晰展示每个成员的模型与运行态）
    out.push(SidebarEntry::plain(String::new()));
    let member_count = state.detail.as_ref().map(|d| d.members.len()).unwrap_or(0);
    out.push(SidebarEntry::plain(section_heading(
        "群组成员",
        &member_count.to_string(),
        member_count == 0,
        width,
        palette,
    )));
    if let Some(detail) = &state.detail {
        for (idx, member) in detail.members.iter().enumerate() {
            let selected = idx == state.selected_member_idx;
            let glyph_str = match (member.role, member.status) {
                (MemberRole::Leader, MemberStatus::Running)
                | (MemberRole::Worker, MemberStatus::Running) => activity_spinner(state.tick),
                (MemberRole::Leader, _) => format!("{}◆{RESET}", fg(palette.accent)),
                (MemberRole::Worker, MemberStatus::Waiting) => {
                    format!("{}◐{RESET}", fg(palette.warning))
                }
                (MemberRole::Worker, MemberStatus::Failed) => {
                    format!("{}×{RESET}", fg(palette.removed))
                }
                (MemberRole::Worker, MemberStatus::Stopped) => {
                    format!("{}○{RESET}", fg(palette.subtle))
                }
                _ => format!("{}●{RESET}", fg(palette.added)),
            };
            let role_name = if member.role == MemberRole::Leader {
                member.name.as_str()
            } else {
                member.name.as_str()
            };
            let model_short = if member.role == MemberRole::Leader {
                state.model_info.leader.short_model.clone()
            } else {
                let stored = short_member_model(&member.model_label);
                if stored.is_empty() {
                    state.model_info.worker.short_model.clone()
                } else {
                    stored
                }
            };
            let name_style = if selected {
                format!("{}{BOLD}", fg(palette.text))
            } else {
                fg(palette.text)
            };
            let left = format!("  {glyph_str} {name_style}{role_name}{RESET}");
            let right_label = if member.status == MemberStatus::Running {
                badge("运行中", palette.accent)
            } else {
                badge(&clip_ansi(&model_short, 14), palette.subtle)
            };
            let mut row = columns(&left, &right_label, width);
            if selected {
                let tint_bg = bg(palette.hover_bg);
                row = format!(
                    "{tint_bg}{}{RESET}",
                    pad_ansi(&row, width).replace(RESET, &format!("{RESET}{tint_bg}"))
                );
            }
            out.push(SidebarEntry::member(row, idx));

            // 选中或运行中成员补充非重复的模型参数与实时活动详情
            if selected || member.status == MemberStatus::Running {
                let activity = member
                    .last_activity
                    .replace(['\n', '\u{3000}'], " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let detail_line = if member.role == MemberRole::Leader {
                    let leader = &state.model_info.leader;
                    let win = crate::cli::repl_chrome::format_token_k(leader.context_window_tokens);
                    if !activity.is_empty() && !matches!(activity.as_str(), "ready" | "idle") {
                        format!("    {} · {activity}", leader.provider_name)
                    } else {
                        format!(
                            "    {} · 思考 {} · 窗口 {win}",
                            leader.provider_name, leader.thinking
                        )
                    }
                } else {
                    let worker = &state.model_info.worker;
                    if !activity.is_empty() && !matches!(activity.as_str(), "ready" | "idle") {
                        format!("    {model_short} · {activity}")
                    } else {
                        format!(
                            "    Runner · {} · {}",
                            worker.provider_name,
                            member_status_label(member.status)
                        )
                    }
                };
                out.push(SidebarEntry::plain(format!(
                    "{}{}{RESET}",
                    fg(palette.subtle),
                    clip_ansi(&detail_line, width)
                )));
            }
        }
    }

    // 3. 任务看板与 Runner 上报
    if let Some(detail) = &state.detail {
        out.push(SidebarEntry::plain(String::new()));
        render_tasks_and_reports(detail, width, palette, out);
    }
}

/// 渲染「任务看板」与「Runner 上报」两个轻量分组。
fn render_tasks_and_reports(
    detail: &GroupDetailSnapshot,
    width: usize,
    palette: &PanelPalette,
    out: &mut Vec<SidebarEntry>,
) {
    out.push(SidebarEntry::plain(section_heading(
        "任务看板",
        &detail.tasks.len().to_string(),
        detail.tasks.is_empty(),
        width,
        palette,
    )));
    if detail.tasks.is_empty() {
        out.push(SidebarEntry::plain(format!(
            "  {}暂无任务，发消息给 Leader 自动派发{RESET}",
            fg(palette.subtle)
        )));
    } else {
        for task in detail.tasks.iter().take(8) {
            let (glyph, color) = match task.status {
                TaskStatus::Delivered => ("✓", palette.added),
                TaskStatus::Running => ("●", palette.accent),
                TaskStatus::WaitingReview => ("◆", palette.warning),
                TaskStatus::Failed | TaskStatus::Blocked => ("×", palette.removed),
                _ => ("○", palette.muted),
            };
            let runner = task.assigned_member_name.as_deref().unwrap_or("未分配");
            let head = format!("  {glyph} {}", task.title);
            out.push(SidebarEntry::plain(format!(
                "{}{}{RESET}",
                fg(color),
                clip_ansi(&head, width)
            )));
            let meta = format!(
                "    {runner} · {} · {}",
                task_status_label(task.status),
                task.origin_channel
            );
            out.push(SidebarEntry::plain(format!(
                "{}{}{RESET}",
                fg(palette.subtle),
                clip_ansi(&meta, width)
            )));
        }
    }

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "Runner 上报",
        &detail.reports.len().to_string(),
        detail.reports.is_empty(),
        width,
        palette,
    )));
    if detail.reports.is_empty() {
        out.push(SidebarEntry::plain(format!(
            "  {}暂无 Runner 报告{RESET}",
            fg(palette.subtle)
        )));
    } else {
        for report in detail.reports.iter().take(6) {
            let status_tag = if report.synthesized_by_leader {
                badge("✓ 已归纳", palette.added)
            } else {
                badge("! 待归纳", palette.warning)
            };
            let left = format!("  {}{BOLD}{}{RESET}", fg(palette.text), report.worker_name);
            out.push(SidebarEntry::plain(columns(&left, &status_tag, width)));
            for wrapped in wrap_plain(
                &report.summary.replace('\n', " "),
                width.saturating_sub(4),
            )
            .into_iter()
            .take(2)
            {
                out.push(SidebarEntry::plain(format!(
                    "    {}{wrapped}{RESET}",
                    fg(palette.muted)
                )));
            }
        }
    }
}

/// 渲染「外部渠道、真实网关状态与微信接入向导」标签页。
fn render_channels_gateways_tab(
    detail: Option<&GroupDetailSnapshot>,
    gw: &GatewayStatusSummary,
    width: usize,
    palette: &PanelPalette,
    out: &mut Vec<SidebarEntry>,
) {
    out.push(SidebarEntry::plain(section_heading(
        "本地网关状态",
        "",
        false,
        width,
        palette,
    )));
    let wx_badge = if gw.weixin_running {
        badge("● 运行中", palette.added)
    } else if gw.weixin_logged_in {
        badge("◐ 待启动", palette.warning)
    } else {
        badge("○ 未登录", palette.subtle)
    };
    let qq_badge = if gw.qq_running {
        badge("● 运行中", palette.added)
    } else if gw.qq_configured {
        badge("◐ 待启动", palette.warning)
    } else {
        badge("○ 未配置", palette.subtle)
    };
    let fs_badge = if gw.feishu_configured {
        badge("● 已配置", palette.added)
    } else {
        badge("○ 未配置", palette.subtle)
    };
    out.push(SidebarEntry::plain(columns(
        &format!("  {}微信 iLink{RESET}", fg(palette.text)),
        &wx_badge,
        width,
    )));
    out.push(SidebarEntry::plain(columns(
        &format!("  {}QQ 机器人{RESET}", fg(palette.text)),
        &qq_badge,
        width,
    )));
    out.push(SidebarEntry::plain(columns(
        &format!("  {}飞书机器人{RESET}", fg(palette.text)),
        &fs_badge,
        width,
    )));
    let route_mode = if gw.channel_leader_routing {
        "自动交由 Leader 编排"
    } else {
        "仅已绑定聊天与 /leader"
    };
    out.push(SidebarEntry::plain(format!(
        "  {}默认路由：{route_mode}{RESET}",
        fg(palette.muted)
    )));

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "微信 / 渠道接入",
        "",
        false,
        width,
        palette,
    )));
    for (step, is_cmd) in [
        ("1. 微信扫码登录：", false),
        ("   sai gateway weixin-login", true),
        ("2. 启动后台网关：", false),
        ("   /gateway start weixin", true),
        ("3. 微信直接发消息对话 Leader", false),
        ("   支持 /group bind 与 /runners", false),
    ] {
        let color = if is_cmd { palette.accent } else { palette.text };
        for wrapped in wrap_plain(step, width.saturating_sub(2)) {
            out.push(SidebarEntry::plain(format!("  {}{wrapped}{RESET}", fg(color))));
        }
    }

    out.push(SidebarEntry::plain(String::new()));
    let bound_count = detail.map(|d| d.channels.len()).unwrap_or(0);
    out.push(SidebarEntry::plain(section_heading(
        "本群绑定渠道",
        &bound_count.to_string(),
        bound_count == 0,
        width,
        palette,
    )));
    if let Some(detail) = detail {
        if detail.channels.is_empty() {
            out.push(SidebarEntry::plain(format!(
                "  {}未绑定（Ctrl+G 或 /bind weixin *）{RESET}",
                fg(palette.subtle)
            )));
        } else {
            for ch in &detail.channels {
                let text = format!("  ● {}:{} [{}]", ch.channel, ch.external_chat_id, ch.admission_mode);
                out.push(SidebarEntry::plain(format!(
                    "{}{}{RESET}",
                    fg(palette.added),
                    clip_ansi(&text, width)
                )));
            }
        }
        out.push(SidebarEntry::plain(String::new()));
        out.push(SidebarEntry::plain(section_heading(
            "最近回推回执",
            &detail.deliveries.len().to_string(),
            detail.deliveries.is_empty(),
            width,
            palette,
        )));
        if detail.deliveries.is_empty() {
            out.push(SidebarEntry::plain(format!(
                "  {}暂无渠道回推记录{RESET}",
                fg(palette.subtle)
            )));
        } else {
            for d in detail.deliveries.iter().take(5) {
                let color = if d.status == "failed" {
                    palette.removed
                } else {
                    palette.added
                };
                let status_label = match d.status.as_str() {
                    "delivered_in_group" => "群内交付",
                    "sent" => "已送达",
                    "simulated" => "模拟回推",
                    "failed" => "发送失败",
                    other => other,
                };
                let preview = clean_preview_text(&d.preview);
                let text = format!("  {} [{status_label}] {preview}", d.target);
                out.push(SidebarEntry::plain(format!(
                    "{}{}{RESET}",
                    fg(color),
                    clip_ansi(&text, width)
                )));
            }
        }
    }
}

/// 渲染「暴露给 Leader 与 Runner 的工具边界」标签页。
fn render_tools_design_tab(
    width: usize,
    palette: &PanelPalette,
    out: &mut Vec<SidebarEntry>,
) {
    let leader_tools = leader_tool_design();
    out.push(SidebarEntry::plain(section_heading(
        "Leader 编排工具",
        &leader_tools.len().to_string(),
        false,
        width,
        palette,
    )));
    for doc in leader_tools {
        let left = format!("  {}{BOLD}{}{RESET}", fg(palette.text), doc.name);
        let right = badge(doc.category, palette.accent);
        out.push(SidebarEntry::plain(columns(&left, &right, width)));
        for wrapped in wrap_plain(doc.summary, width.saturating_sub(4)) {
            out.push(SidebarEntry::plain(format!(
                "    {}{wrapped}{RESET}",
                fg(palette.muted)
            )));
        }
    }

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "Leader 只读感知",
        &LEADER_READONLY_TOOLS.len().to_string(),
        false,
        width,
        palette,
    )));
    let ro = LEADER_READONLY_TOOLS.join(", ");
    for wrapped in wrap_plain(&ro, width.saturating_sub(2)) {
        out.push(SidebarEntry::plain(format!(
            "  {}{wrapped}{RESET}",
            fg(palette.text)
        )));
    }

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "Leader 隔离禁用",
        "",
        false,
        width,
        palette,
    )));
    for wrapped in wrap_plain(
        "run_command, background_command, write_file, str_replace, apply_patch, trash_path, browser_*, computer_*, ssh_*, subagent",
        width.saturating_sub(2),
    ) {
        out.push(SidebarEntry::plain(format!(
            "  {}{wrapped}{RESET}",
            fg(palette.muted)
        )));
    }

    out.push(SidebarEntry::plain(String::new()));
    let worker_tools = worker_tool_design();
    out.push(SidebarEntry::plain(section_heading(
        "Worker 协作工具",
        &worker_tools.len().to_string(),
        false,
        width,
        palette,
    )));
    for doc in worker_tools {
        let left = format!("  {}{BOLD}{}{RESET}", fg(palette.text), doc.name);
        let right = badge(doc.category, palette.warning);
        out.push(SidebarEntry::plain(columns(&left, &right, width)));
        for wrapped in wrap_plain(doc.summary, width.saturating_sub(4)) {
            out.push(SidebarEntry::plain(format!(
                "    {}{wrapped}{RESET}",
                fg(palette.muted)
            )));
        }
    }
}

/// 渲染「线程订阅、路由触发器与 Watchdog 守护」标签页。
fn render_threads_watchdog_tab(
    detail: &GroupDetailSnapshot,
    watchdog_events: &[WatchdogEvent],
    width: usize,
    palette: &PanelPalette,
    out: &mut Vec<SidebarEntry>,
) {
    out.push(SidebarEntry::plain(section_heading(
        "线程订阅",
        &detail.subscriptions.len().to_string(),
        detail.subscriptions.is_empty(),
        width,
        palette,
    )));
    if detail.subscriptions.is_empty() {
        out.push(SidebarEntry::plain(format!(
            "  {}暂无线程订阅{RESET}",
            fg(palette.subtle)
        )));
    } else {
        for sub in detail.subscriptions.iter().take(6) {
            let member_name = detail
                .members
                .iter()
                .find(|m| m.id == sub.member_id)
                .map(|m| m.name.as_str())
                .unwrap_or(&sub.member_id);
            let text = format!("  ● {member_name} [{}] → {}", sub.mode.as_str(), sub.thread_id);
            out.push(SidebarEntry::plain(format!(
                "{}{}{RESET}",
                fg(palette.text),
                clip_ansi(&text, width)
            )));
        }
    }

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "路由审计 Trigger",
        &detail.triggers.len().to_string(),
        detail.triggers.is_empty(),
        width,
        palette,
    )));
    for trig in detail.triggers.iter().take(8) {
        let text = format!(
            "  [{}] {}: {}",
            trig.route_path,
            trig.source_channel,
            trig.content.replace('\n', " ")
        );
        out.push(SidebarEntry::plain(format!(
            "{}{}{RESET}",
            fg(palette.muted),
            clip_ansi(&text, width)
        )));
    }

    out.push(SidebarEntry::plain(String::new()));
    out.push(SidebarEntry::plain(section_heading(
        "Watchdog 巡检",
        &watchdog_events.len().to_string(),
        watchdog_events.is_empty(),
        width,
        palette,
    )));
    for ev in watchdog_events.iter().take(6) {
        let color = match ev.severity {
            WatchdogSeverity::Healthy | WatchdogSeverity::Recovered => palette.added,
            WatchdogSeverity::Notice => palette.warning,
            WatchdogSeverity::ActionRequired => palette.removed,
        };
        let text = format!("  [{}] {}", ev.severity.as_str(), ev.summary);
        out.push(SidebarEntry::plain(format!(
            "{}{}{RESET}",
            fg(color),
            clip_ansi(&text, width)
        )));
    }
}
