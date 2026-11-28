//! 群组右侧统一侧栏容器：完全对齐 Sai 全屏右侧栏风格（对称留白、目录/分支双行顶栏、CTX 分段条与模型摘要、
//! 无边框标签切换栏、可滚动正文与底部操作提示）。

use super::inspector_view::render_tab_content;
use super::state::{GroupTuiState, InspectorTab};
use super::text_util::{
    badge, clip_ansi, columns, compact_directory, compact_tokens, pad_ansi, scrollbar_column,
    style_directory, BOLD, RESET,
};
use crate::cli::repl_text::visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use crate::super_agent::model::{MemberRole, MemberStatus};

/// 【群组终端】【右侧统一侧栏】绘制覆盖整屏高度的 Sai 风格右侧概览与检视面板。
///
/// 参数:
/// - `state`: 群组 TUI 状态（写入群组/成员/标签鼠标点击热区）
/// - `sidebar_width`: 侧栏含左分界线 `│` 的总列宽
/// - `total_rows`: 终端总行数
/// - `sidebar_start_col`: 侧栏左分界线在屏幕中的起始列号
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 长度等于 `total_rows`、每行宽 `sidebar_width`（含左侧 `│`）的完整侧栏行
pub(super) fn render_right_sidebar(
    state: &mut GroupTuiState,
    sidebar_width: usize,
    total_rows: usize,
    sidebar_start_col: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    if total_rows == 0 || sidebar_width < 8 {
        return vec![String::new(); total_rows];
    }

    // 1. 计算与主全屏侧栏一致的横向对称留白（扣除 1 列左分界线）
    let usable = sidebar_width.saturating_sub(1);
    let side_pad = if usable >= 36 { 2usize } else { 1usize };
    let content_w = usable.saturating_sub(side_pad * 2).max(12);
    let content_col_start = sidebar_start_col + 1 + side_pad;
    let left_pad = " ".repeat(side_pad);
    let right_pad = " ".repeat(usable.saturating_sub(side_pad + content_w));

    let mut inner_rows: Vec<String> = Vec::with_capacity(total_rows);

    // 2. 顶部工作区目录与 Git 分支（对齐主全屏侧栏：第 0 行避让右上角 ◫，高屏第 1 行呼吸留白）
    if total_rows >= 20 {
        inner_rows.push(String::new());
    }
    if total_rows >= 26 {
        inner_rows.push(String::new());
    }
    let dir_budget = if total_rows >= 20 {
        content_w.saturating_sub(2).max(6)
    } else {
        content_w.saturating_sub(6).max(6)
    };
    let short_dir = compact_directory(&state.model_info.directory, dir_budget);
    inner_rows.push(format!(
        "{}◆ {}{RESET}",
        fg(palette.accent),
        style_directory(&short_dir, palette)
    ));
    if total_rows >= 18 {
        inner_rows.push(render_branch_row(
            state.model_info.branch.as_deref(),
            content_w,
            palette,
        ));
    }

    // 3. 模型与上下文摘要卡（直接展示 Leader / Worker 模型、思考等级、上下文进度条与并发；点击 Leader 行切换模型）
    if total_rows >= 22 {
        inner_rows.push(String::new());
    }
    let metrics_start_row = inner_rows.len();
    inner_rows.extend(render_model_metrics_block(
        state,
        metrics_start_row,
        sidebar_start_col,
        content_w,
        total_rows,
        palette,
    ));

    // 4. 紧凑无框标签切换栏（总览 / 渠道 / 工具 / 守护）
    if total_rows >= 20 {
        inner_rows.push(String::new());
    }
    let tab_row = inner_rows.len();
    inner_rows.push(render_tab_strip(
        state,
        tab_row,
        content_col_start,
        content_w,
        palette,
    ));
    inner_rows.push(format!(
        "{}{}{RESET}",
        fg(palette.border),
        "─".repeat(content_w)
    ));

    // 5. 可滚动标签正文 + 细滚动条（正文与滑块之间保留 1 列留白，底部保留 1 行静默快捷键提示）
    let footer_reserved = usize::from(total_rows >= 14);
    let body_start_row = inner_rows.len();
    let body_h = total_rows
        .saturating_sub(body_start_row + footer_reserved)
        .max(1);
    let body_text_w = content_w.saturating_sub(2).max(10);
    state.hit_map.sidebar_body_rows = (body_start_row as u16, (body_start_row + body_h) as u16);

    let body_all = render_tab_content(state, body_text_w, palette);
    let total_body = body_all.len();
    let max_scroll = total_body.saturating_sub(body_h);
    let scroll = state.inspector_scroll.min(max_scroll);
    state.inspector_scroll = scroll;
    let sb = scrollbar_column(body_h, total_body, scroll, palette);

    for offset in 0..body_h {
        let screen_row = (body_start_row + offset) as u16;
        let Some(entry) = body_all.get(scroll + offset) else {
            inner_rows.push(" ".repeat(content_w));
            continue;
        };
        if let Some(group_idx) = entry.group_idx {
            state.hit_map.group_rows.push((screen_row, group_idx));
        }
        if let Some(member_idx) = entry.member_idx {
            state.hit_map.member_rows.push((screen_row, member_idx));
        }
        let hovered = (entry.group_idx.is_some() || entry.member_idx.is_some())
            && state.pointer.is_some_and(|(col, row)| {
                row == screen_row && usize::from(col) > sidebar_start_col
            });
        let text = if hovered {
            crate::cli::repl_runtime::control_tint_cols(&entry.text, 0, body_text_w)
        } else {
            entry.text.clone()
        };
        let glyph = sb.get(offset).map(String::as_str).unwrap_or(" ");
        inner_rows.push(format!("{} {glyph}", pad_ansi(&text, body_text_w)));
    }

    if footer_reserved > 0 {
        inner_rows.push(render_sidebar_footer(content_w, palette));
    }
    while inner_rows.len() < total_rows {
        inner_rows.push(String::new());
    }
    inner_rows.truncate(total_rows);

    // 6. 拼装左侧单列竖线 `│` 与对称边距
    let divider = format!("{}│{RESET}", fg(palette.border));
    inner_rows
        .into_iter()
        .map(|row| format!("{divider}{left_pad}{}{right_pad}", pad_ansi(&row, content_w)))
        .collect()
}

/// 渲染工作区 Git 分支行（对齐 `sidebar_header_path.rs`）。
fn render_branch_row(branch: Option<&str>, width: usize, palette: &PanelPalette) -> String {
    let clean = branch.unwrap_or_default().trim();
    let budget = width.saturating_sub(4);
    if clean.is_empty() {
        return format!(
            "  {}· {}{RESET}",
            fg(palette.subtle),
            clip_ansi("本地目录", budget)
        );
    }
    let icon = if crate::render::nerd_font::nerd_font_enabled() {
        "\u{e0a0}"
    } else {
        "⊢"
    };
    format!(
        "  {}{icon} {}{}{RESET}",
        fg(palette.accent),
        fg(palette.muted),
        clip_ansi(clean, budget)
    )
}

/// 渲染侧栏顶部的模型基础信息与上下文占用分段条（对齐 `sidebar_metrics.rs` + `sidebar_context_bar.rs`）。
fn render_model_metrics_block(
    state: &mut GroupTuiState,
    start_row: usize,
    sidebar_start_col: usize,
    width: usize,
    total_rows: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let leader = state.model_info.leader.clone();
    let worker = state.model_info.worker.clone();
    let mut rows = Vec::with_capacity(4);

    // 1. CTX 容量标题与距压缩余量
    let used = leader.context_used_tokens;
    let window = leader.context_window_tokens.max(1);
    let trigger = leader.compaction_trigger_tokens.min(window);
    let ctx_left = format!(
        "{}CTX {}{}{}/{}\x1b[39m",
        fg(palette.muted),
        fg(palette.text),
        compact_tokens(used),
        fg(palette.subtle),
        compact_tokens(window)
    );
    let tone = if used >= trigger {
        palette.removed
    } else if used as f64 >= trigger as f64 * 0.85 {
        palette.warning
    } else {
        palette.accent
    };
    let ctx_right = if let Some(cache) = leader.cache_hit_ratio {
        format!(
            "{}缓存 {}{:.0}%{RESET}",
            fg(palette.muted),
            fg(palette.added),
            (cache * 100.0).clamp(0.0, 100.0)
        )
    } else {
        let pct = (leader.context_ratio * 100.0).clamp(0.0, 999.9);
        format!("{}{pct:.1}%{RESET}", fg(tone))
    };
    rows.push(columns(&ctx_left, &ctx_right, width));

    // 2. 正常高度下显示连续色段轨道 `▰▰▱▱...`
    if total_rows >= 26 {
        rows.push(render_context_track(
            used,
            window,
            trigger,
            leader.cache_hit_ratio,
            width,
            palette,
        ));
    }

    // 3. Leader 模型基本信息（短名 + 思考等级 + 覆盖标注；支持鼠标悬停与点击呼出 `/model`）
    let leader_screen_row = (start_row + rows.len()) as u16;
    state.hit_map.sidebar_model_row = Some(leader_screen_row);
    let leader_left = format!(
        "{}Leader {}{}{RESET}",
        fg(palette.muted),
        fg(palette.text),
        clip_ansi(&leader.short_model, width.saturating_sub(15).max(8))
    );
    let leader_right_text = if leader.is_override {
        format!("{} · 独立", leader.thinking)
    } else {
        leader.thinking.clone()
    };
    let leader_right = badge(&leader_right_text, palette.accent);
    let mut leader_row = columns(&leader_left, &leader_right, width);
    let leader_hovered = state.pointer.is_some_and(|(col, row)| {
        row == leader_screen_row && usize::from(col) > sidebar_start_col
    });
    if leader_hovered {
        leader_row = crate::cli::repl_runtime::control_tint_cols(&leader_row, 0, width);
    }
    rows.push(leader_row);

    // 4. Worker 模型与并发槽位信息
    let running_w = state
        .detail
        .as_ref()
        .map(|detail| {
            detail
                .members
                .iter()
                .filter(|m| m.role == MemberRole::Worker && m.status == MemberStatus::Running)
                .count()
        })
        .unwrap_or(0);
    let worker_desc = if state.model_info.worker_shares_leader {
        "同 Leader".to_string()
    } else {
        format!("{} · {}", worker.short_model, worker.thinking)
    };
    let worker_left = format!(
        "{}Worker {}{}{RESET}",
        fg(palette.muted),
        fg(if state.model_info.worker_shares_leader {
            palette.subtle
        } else {
            palette.text
        }),
        clip_ansi(&worker_desc, width.saturating_sub(14).max(8))
    );
    let slot_color = if running_w > 0 {
        palette.added
    } else {
        palette.muted
    };
    let worker_right = badge(
        &format!("{running_w}/{}", state.model_info.max_concurrent_workers),
        slot_color,
    );
    rows.push(columns(&worker_left, &worker_right, width));

    rows
}

/// 绘制与 `sidebar_context_bar.rs` 一致的连续分段容量轨道。
fn render_context_track(
    used: usize,
    window: usize,
    trigger: usize,
    cache_ratio: Option<f32>,
    width: usize,
    palette: &PanelPalette,
) -> String {
    if width == 0 || window == 0 {
        return String::new();
    }
    let used = used.min(window);
    let reserve_start = (trigger as f64 / window as f64 * width as f64).round() as usize;
    let filled = if used == 0 {
        0
    } else {
        ((used as f64 / window as f64 * width as f64).ceil() as usize).min(width)
    };
    let cached_cells = cache_ratio.map_or(0, |ratio| {
        ((filled as f64) * (ratio.clamp(0.0, 1.0) as f64)).round() as usize
    });
    let mut track = String::new();
    for col in 0..width {
        let (glyph, color) = if col < filled {
            let color = if col >= reserve_start {
                palette.removed
            } else if col < cached_cells {
                palette.added
            } else {
                palette.accent
            };
            ('▰', color)
        } else if col >= reserve_start {
            ('▱', palette.warning)
        } else {
            ('▱', palette.subtle)
        };
        track.push_str(&fg(color));
        track.push(glyph);
    }
    track.push_str(RESET);
    track
}

/// 渲染右侧栏顶部无框标签栏，并记录各标签点击列范围。
fn render_tab_strip(
    state: &mut GroupTuiState,
    screen_row: usize,
    content_col_start: usize,
    width: usize,
    palette: &PanelPalette,
) -> String {
    let mut out = String::new();
    let mut cursor_col = content_col_start;
    for (idx, tab) in InspectorTab::all().into_iter().enumerate() {
        if idx > 0 {
            out.push_str("  ");
            cursor_col += 2;
        }
        let label = tab.label();
        let active = tab == state.inspector_tab;
        let item_w = visible_width(label) + usize::from(active) * 2;
        let start = cursor_col as u16;
        let end = (cursor_col + item_w) as u16;
        state
            .hit_map
            .tab_hits
            .push((screen_row as u16, start, end, tab));
        let hovered = state
            .pointer
            .is_some_and(|(col, row)| row == screen_row as u16 && col >= start && col < end);
        if active {
            out.push_str(&format!(
                "{}● {}{BOLD}{label}{RESET}",
                fg(palette.accent),
                fg(palette.text)
            ));
        } else if hovered {
            out.push_str(&format!("{}{label}{RESET}", fg(palette.accent)));
        } else {
            out.push_str(&format!("{}{label}{RESET}", fg(palette.muted)));
        }
        cursor_col += item_w;
    }
    pad_ansi(&out, width)
}

/// 渲染侧栏最底行静默操作提示（对齐 `sidebar_style::footer`）。
fn render_sidebar_footer(width: usize, palette: &PanelPalette) -> String {
    let keys = [
        ("Tab", "切页"),
        ("Alt+↑↓", "切人"),
        ("F2", "收起"),
    ];
    let mut line = String::new();
    let mut used = 0usize;
    for (idx, (key, label)) in keys.iter().enumerate() {
        let sep = if idx == 0 { "" } else { " · " };
        let piece_w = visible_width(sep) + visible_width(key) + 1 + visible_width(label);
        if used + piece_w > width {
            break;
        }
        line.push_str(&format!(
            "{}{sep}{}{key} {}{label}",
            fg(palette.muted),
            fg(palette.text),
            fg(palette.muted)
        ));
        used += piece_w;
    }
    format!("{line}{RESET}")
}
