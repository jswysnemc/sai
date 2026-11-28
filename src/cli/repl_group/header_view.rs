//! 群组主画布顶栏：对齐 Sai 全屏浮动标题栏与右上角侧栏开关的视觉语法。

use super::state::GroupTuiState;
use super::text_util::{activity_spinner, clip_ansi, header_bg, BOLD, RESET};
use crate::cli::repl_text::visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use crate::super_agent::model::{MemberRole, MemberStatus, WatchdogSeverity};

/// Nerd Font 分栏图标（与 `fullscreen/header_controls.rs` 保持一致）。
const SIDEBAR_ICON_NERD: &str = "\u{f0db}";
/// 禁用 Nerd Font 时的单列几何分栏图标。
const SIDEBAR_ICON_FALLBACK: &str = "◫";
/// 侧栏开关按钮占用列宽（含左右单格留白）。
const TOGGLE_WIDTH: usize = 3;

/// 【群组终端】【顶栏渲染】绘制主画布第 0 行单行浮动标题栏（对齐 `fullscreen/paint.rs`）。
///
/// 参数:
/// - `state`: 群组 TUI 状态
/// - `main_width`: 左侧主画布总宽度
/// - `reserve_sidebar_toggle`: 是否在主画布最右端预留侧栏开关 `◫` 占位（仅宽屏且侧栏收起时为 true）
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 恰好占用 `main_width` 列的顶栏 ANSI 行
pub(super) fn render_main_header(
    state: &GroupTuiState,
    main_width: usize,
    reserve_sidebar_toggle: bool,
    palette: &PanelPalette,
) -> String {
    let bar_bg = header_bg(palette);
    let title = state
        .detail
        .as_ref()
        .map(|d| d.group.name.as_str())
        .unwrap_or("未选择群组");

    let (leader_running, total_w, running_w, alerts) = match &state.detail {
        Some(detail) => {
            let leader_run = state.leader_live.thinking
                || detail
                    .members
                    .iter()
                    .find(|m| m.role == MemberRole::Leader)
                    .is_some_and(|m| m.status == MemberStatus::Running);
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
            (leader_run, total_w, running_w, alerts)
        }
        None => (false, 0, 0, 0),
    };

    // 1. 左侧：圆点锚点 + 群组名 + 轻量态摘要（无实心胶囊堆叠）
    let anchor = if leader_running {
        activity_spinner(state.tick)
    } else {
        format!("{}●{RESET}", fg(palette.accent))
    };
    let mut left_segments = vec![format!(
        "{anchor}{bar_bg} {}{BOLD}{title}\x1b[22m{RESET}",
        fg(palette.text)
    )];
    if leader_running {
        left_segments.push(format!("{}Leader 编排中{RESET}", fg(palette.accent)));
    } else if total_w > 0 {
        let tone = if running_w > 0 {
            palette.added
        } else {
            palette.muted
        };
        left_segments.push(format!("{}Runners {running_w}/{total_w}{RESET}", fg(tone)));
    }
    if state.gateway_status.weixin_running {
        left_segments.push(format!("{}微信在线{RESET}", fg(palette.added)));
    }
    if alerts > 0 {
        left_segments.push(format!("{}告警 {alerts}{RESET}", fg(palette.removed)));
    }
    let sep = format!("{bar_bg} {}·{RESET}{bar_bg} ", fg(palette.subtle));
    let left_raw = format!("{bar_bg} {}", left_segments.join(&sep));

    // 2. 右侧：紧凑滚动位置指示（对齐 `header_controls::position`）
    let right_raw = scroll_position_indicator(state, main_width, palette);

    // 3. 仅当宽屏允许展开侧栏且当前处于收起态时预留最右端 `◫` 按钮列宽，窄屏直接贴齐右边缘
    let reserve_toggle = usize::from(reserve_sidebar_toggle && main_width >= 24) * (TOGGLE_WIDTH + 1);
    let usable = main_width.saturating_sub(reserve_toggle);
    let right_w = visible_width(&right_raw);
    let left_budget = usable.saturating_sub(right_w + 3).max(1);
    let left_clipped = clip_ansi(&left_raw, left_budget);
    let left_rebg = left_clipped.replace(RESET, &format!("{RESET}{bar_bg}"));
    let right_rebg = right_raw.replace(RESET, &format!("{RESET}{bar_bg}"));
    let left_w = visible_width(&left_clipped);
    let gap = usable.saturating_sub(left_w + right_w + 1);
    let mut line = format!(
        "{bar_bg}{left_rebg}{bar_bg}{}{right_rebg}{bar_bg} {RESET}",
        " ".repeat(gap)
    );
    if reserve_toggle > 0 {
        line.push_str(&format!("{bar_bg}{}{RESET}", " ".repeat(reserve_toggle)));
    }
    clip_ansi(&line, main_width)
}

/// 【群组终端】【侧栏开关】在整屏第 0 行最右端叠加可点击的侧栏切换图标 `◫`（对齐 `header_controls::overlay`）。
///
/// 参数:
/// - `row0`: 整屏第 0 行文本
/// - `total_cols`: 终端总列数
/// - `sidebar_visible`: 侧栏当前是否展开
/// - `can_open_sidebar`: 当前终端宽度是否允许展开侧栏
/// - `pointer`: 当前鼠标坐标
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 若渲染了按钮则返回其列区间 `Some((start_col, end_col))`
pub(super) fn overlay_sidebar_toggle(
    row0: &mut String,
    total_cols: usize,
    sidebar_visible: bool,
    can_open_sidebar: bool,
    pointer: Option<(u16, u16)>,
    palette: &PanelPalette,
) -> Option<(u16, u16)> {
    if !can_open_sidebar || total_cols < 24 {
        return None;
    }
    let end = total_cols.saturating_sub(1);
    let start = end.saturating_sub(TOGGLE_WIDTH);
    let hovered = pointer.is_some_and(|(col, row)| {
        row == 0 && usize::from(col) >= start && usize::from(col) < end
    });
    let icon = if crate::render::nerd_font::nerd_font_enabled() {
        SIDEBAR_ICON_NERD
    } else {
        SIDEBAR_ICON_FALLBACK
    };
    let btn_bg = if sidebar_visible {
        "\x1b[49m".to_string()
    } else {
        header_bg(palette)
    };
    let color = fg(if hovered {
        palette.accent
    } else {
        palette.text
    });
    let left = crate::render::ansi_clip::clip_ansi_to_width(row0, start, "");
    let pad = start.saturating_sub(visible_width(&left));
    *row0 = format!(
        "{left}{btn_bg}{}{color} {icon}  {RESET}",
        " ".repeat(pad)
    );
    Some((start as u16, end as u16))
}

/// 构造主画布顶栏右侧紧凑滚动位置轨道（对齐 `fullscreen/header_controls.rs`）。
fn scroll_position_indicator(
    state: &GroupTuiState,
    main_width: usize,
    palette: &PanelPalette,
) -> String {
    if state.chat_total_lines == 0 {
        return String::new();
    }
    let max_scroll = state
        .chat_total_lines
        .saturating_sub(state.chat_viewport_rows.max(1));
    let percent = if max_scroll == 0 {
        100usize
    } else {
        let clamped = state.chat_scroll.min(max_scroll);
        ((max_scroll - clamped) * 100) / max_scroll
    };
    let color = fg(if state.unseen_chat {
        palette.accent
    } else {
        palette.muted
    });
    if main_width < 68 {
        return format!("{color}{percent}%{RESET}");
    }
    let marker = percent * 4 / 100;
    let track: String = (0..5)
        .map(|idx| if idx == marker { '●' } else { '─' })
        .collect();
    format!("{color}{track} {percent:>3}%{RESET}")
}
