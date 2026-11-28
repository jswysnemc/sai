//! 群组原生输入区：直接复用 Sai 主 REPL 的 `repl_chrome` 输入条（支持 Panel/Separator 样式、明暗主题、多行折行、硬件光标）、
//! 透明内联 `/命令` 与 `@成员` 补全面板、模型基础信息底栏与上下文敏感按键提示行。

use super::state::{CompletionItem, GroupTuiState};
use super::text_util::{clip_ansi, pad_ansi, BOLD, DIM, RESET};
use crate::cli::repl_chrome::{
    chrome_input_content_cols, chrome_input_pad_row, chrome_input_row, colorize_right_status,
    fit_status_segments, ChromeInputPrefix, CHROME_FOOTER_SIDE_PAD, CHROME_INPUT_PREFIX_COLS,
};
use crate::cli::repl_input_render::repl_cursor_position_for_cols;
use crate::cli::repl_text::{char_terminal_width, repl_input_lines, visible_width};
use crate::config::InputStyle;
use crate::render::panel_palette::{fg, PanelPalette};

/// 输入框最多同时展示的视觉正文行数（超出时随光标垂直滚动视口）。
const MAX_VISIBLE_INPUT_ROWS: usize = 6;
/// 补全面板最多同时展示的候选行数。
const MAX_COMPLETION_ROWS: usize = 6;

/// 原生输入区单帧渲染输出（含渲染行与相对输入区顶部的真实硬件光标坐标）。
pub(super) struct ComposerFrameOutput {
    pub(super) lines: Vec<String>,
    /// `(col, relative_row)`，相对本输入区第 0 行
    pub(super) cursor: (u16, u16),
}

/// 【群组终端】【原生输入区】使用 Sai 原生 `repl_chrome` 样式渲染多行输入条、内联补全面板、模型底栏与按键提示行。
///
/// 参数:
/// - `state`: 群组 TUI 状态（同步写入补全与模型按钮点击热区）
/// - `width`: 左侧主画布列宽
/// - `max_rows`: 允许输入区占用的最大总行数（确保上方群聊正文保留可读空间）
/// - `composer_top_row`: 输入区在整屏中的起始行号
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 输入区完整行数组与真实光标位置
pub(super) fn render_native_composer(
    state: &mut GroupTuiState,
    width: usize,
    max_rows: usize,
    composer_top_row: usize,
    palette: &PanelPalette,
) -> ComposerFrameOutput {
    let width = width.max(20);
    let content_cols = chrome_input_content_cols(width);
    state.hit_map.composer_content_cols = content_cols;

    // 1. 按 Sai 原生折行规则计算逻辑行、视觉行与真实光标位置
    let completions = state.completion_items();
    let has_completions = !completions.is_empty();
    let is_slash = has_completions && state.input.starts_with('/');
    let show_comp_header = is_slash && completions.len() > 1;
    let comp_rows_wanted = if has_completions {
        completions.len().min(MAX_COMPLETION_ROWS) + usize::from(show_comp_header)
    } else {
        0
    };
    let footer_and_hints = if has_completions {
        comp_rows_wanted + 1
    } else {
        2
    };
    let max_input_rows = max_rows
        .saturating_sub(2 + footer_and_hints)
        .clamp(1, MAX_VISIBLE_INPUT_ROWS);

    let (cursor_col_rel, cursor_row_all) =
        repl_cursor_position_for_cols("", &state.input, state.cursor, content_cols);
    let visual_lines = if state.input.is_empty() {
        vec![placeholder_line(content_cols)]
    } else {
        repl_input_lines(&state.input)
            .iter()
            .flat_map(|line| wrap_input_line(line, content_cols))
            .collect::<Vec<_>>()
    };
    let (visible_input_lines, cursor_row_in_box) =
        select_input_rows(&visual_lines, usize::from(cursor_row_all), max_input_rows);

    // 2. 绘制 Sai 原生输入条（遵循用户配置的 Panel 抬升面板或 Separator 简约细线）
    let style = state.model_info.input_style;
    let mut lines = Vec::with_capacity(visible_input_lines.len() + 2 + footer_and_hints);
    lines.push(render_input_padding(style, width, true, palette));
    let input_body_start = lines.len();
    for (idx, text) in visible_input_lines.iter().enumerate() {
        let prefix = if idx == 0 {
            ChromeInputPrefix::Message
        } else {
            ChromeInputPrefix::Continuation
        };
        lines.push(render_input_row(style, prefix, text, width));
    }
    lines.push(render_input_padding(style, width, false, palette));

    // 3. 输入条下方：补全面板激活时原地展开透明候选列表，否则显示 Sai 同款模型与工作区状态底栏
    let suppress_hint_row = if has_completions {
        let comp_budget = max_rows.saturating_sub(lines.len() + 1).max(1);
        render_inline_completions(
            &mut lines,
            state,
            &completions,
            is_slash,
            width,
            comp_budget,
            composer_top_row,
            palette,
        )
    } else {
        let status_screen_row = (composer_top_row + lines.len()) as u16;
        lines.push(render_model_status_footer(
            state,
            width,
            status_screen_row,
            palette,
        ));
        false
    };

    // 4. 末行固定为透明按键提示行（带顶栏的斜杠面板已自带说明时留空，对齐主 REPL）
    lines.push(if suppress_hint_row {
        " ".repeat(width)
    } else {
        render_key_hints_row(state, has_completions, width, palette)
    });

    let cursor_col = cursor_col_rel
        .saturating_add(CHROME_INPUT_PREFIX_COLS as u16)
        .min(width.saturating_sub(1) as u16);
    let cursor_row = (input_body_start + usize::from(cursor_row_in_box)) as u16;
    ComposerFrameOutput {
        lines,
        cursor: (cursor_col, cursor_row),
    }
}

/// 预估当前输入区在指定宽度与高度预算下需要的总视觉行数，供主布局分配群聊视口高度。
///
/// 参数:
/// - `state`: 群组 TUI 状态
/// - `width`: 左侧主画布列宽
/// - `max_rows`: 输入区最大允许总行数
///
/// 返回:
/// - 本帧输入区占用的精确行数
pub(super) fn measure_composer_height(
    state: &GroupTuiState,
    width: usize,
    max_rows: usize,
) -> usize {
    let width = width.max(20);
    let content_cols = chrome_input_content_cols(width);
    let completions = state.completion_items();
    let has_completions = !completions.is_empty();
    let is_slash = has_completions && state.input.starts_with('/');
    let show_comp_header = is_slash && completions.len() > 1;
    let comp_rows = if has_completions {
        completions.len().min(MAX_COMPLETION_ROWS) + usize::from(show_comp_header)
    } else {
        0
    };
    let footer_and_hints = if has_completions { comp_rows + 1 } else { 2 };
    let max_input_rows = max_rows
        .saturating_sub(2 + footer_and_hints)
        .clamp(1, MAX_VISIBLE_INPUT_ROWS);
    let input_rows = if state.input.is_empty() {
        1
    } else {
        repl_input_lines(&state.input)
            .iter()
            .map(|line| wrap_input_line(line, content_cols).len())
            .sum::<usize>()
            .clamp(1, max_input_rows)
    };
    let below_rows = if has_completions {
        let budget = max_rows.saturating_sub(input_rows + 3).max(1);
        comp_rows.min(budget) + 1
    } else {
        2
    };
    (input_rows + 2 + below_rows).min(max_rows.max(5))
}

/// 渲染输入区上下内边距或简约分割线（完全对齐 `composer_frame/input_style.rs`）。
fn render_input_padding(
    style: InputStyle,
    width: usize,
    top: bool,
    palette: &PanelPalette,
) -> String {
    match style {
        InputStyle::Panel => chrome_input_pad_row(width),
        InputStyle::Separator if top => {
            format!("\x1b[49m{}{}{RESET}", fg(palette.border), "─".repeat(width))
        }
        InputStyle::Separator => " ".repeat(width),
    }
}

/// 渲染单行原生输入正文（Panel 直接调用 `repl_chrome::chrome_input_row`，Separator 对齐 `input_style.rs`）。
fn render_input_row(
    style: InputStyle,
    prefix: ChromeInputPrefix,
    content: &str,
    width: usize,
) -> String {
    match style {
        InputStyle::Panel => chrome_input_row(prefix, content, width),
        InputStyle::Separator => {
            let text = crate::render::ansi_clip::clip_ansi_to_width(
                content,
                chrome_input_content_cols(width),
                "",
            );
            let row = format!("{}{text}", prefix.render());
            format!(
                "\x1b[49m{row}{}{RESET}",
                " ".repeat(width.saturating_sub(visible_width(&row)))
            )
        }
    }
}

/// 按可用列宽返回简洁占位提示（不重复底栏快捷键）。
fn placeholder_line(width: usize) -> String {
    let candidates = [
        "输入消息给 Leader，@ 选成员，/ 选命令",
        "输入消息，@ 成员或 / 命令",
        "输入消息或 /help",
        "输入消息",
    ];
    let text = candidates
        .into_iter()
        .find(|candidate| visible_width(candidate) <= width)
        .unwrap_or("");
    format!("{DIM}{text}{RESET}")
}

/// 渲染输入条下方的透明内联补全面板（完全对齐 `slash_panel/render.rs` 与 `mention_panel.rs`），
/// 返回是否渲染了自带操作说明的顶栏。
#[allow(clippy::too_many_arguments)]
fn render_inline_completions(
    lines: &mut Vec<String>,
    state: &mut GroupTuiState,
    items: &[CompletionItem],
    is_slash: bool,
    width: usize,
    budget: usize,
    composer_top_row: usize,
    palette: &PanelPalette,
) -> bool {
    let show_header = is_slash && items.len() > 1 && budget >= 2;
    let visible_items = if show_header {
        budget.saturating_sub(1)
    } else {
        budget
    }
    .min(MAX_COMPLETION_ROWS)
    .min(items.len())
    .max(1);
    let active = state.completion_idx.min(items.len() - 1);
    let start = active
        .saturating_sub(visible_items / 2)
        .min(items.len().saturating_sub(visible_items));

    state.hit_map.completion_end_col = width as u16;
    if show_header {
        lines.push(render_completion_header(
            start,
            visible_items,
            items.len(),
            width,
            palette,
        ));
    }

    let cmd_width = items[start..start + visible_items]
        .iter()
        .map(|item| visible_width(&item.label))
        .max()
        .unwrap_or(10)
        .saturating_add(2)
        .clamp(10, 20)
        .min(width.saturating_sub(6));

    for (offset, item) in items.iter().skip(start).take(visible_items).enumerate() {
        let idx = start + offset;
        let screen_row = (composer_top_row + lines.len()) as u16;
        state.hit_map.completion_hits.push((screen_row, idx));
        let hovered = state
            .pointer
            .is_some_and(|(col, row)| row == screen_row && usize::from(col) < width);
        let mut row = format_completion_row(item, cmd_width, width, idx == active, palette);
        if hovered && idx != active {
            row = crate::cli::repl_runtime::control_tint_cols(&row, 0, width);
        }
        lines.push(pad_ansi(&row, width));
    }
    show_header
}

/// 渲染斜杠命令内联顶栏（与 `slash_panel/render.rs::header_line` 视觉一致）。
fn render_completion_header(
    start: usize,
    visible: usize,
    total: usize,
    cols: usize,
    palette: &PanelPalette,
) -> String {
    let up = if start > 0 { "▲" } else { "" };
    let down = if start + visible < total { "▼" } else { "" };
    let arrows = format!("{up}{down}");
    let range = if arrows.is_empty() {
        format!("{}-{}/{}", start + 1, start + visible, total)
    } else {
        format!("{}-{}/{} {arrows}", start + 1, start + visible, total)
    };
    let title = format!(
        " {}◆{RESET} {}{BOLD}快捷命令{RESET} {}· {range}{RESET}",
        fg(palette.accent),
        fg(palette.text),
        fg(palette.muted),
    );
    let hint = format!("{}↑↓ 选择 · Tab 补全 · Esc 关闭{RESET}", fg(palette.muted));
    let title_w = visible_width(&title);
    let hint_w = visible_width(&hint);
    if cols >= title_w + hint_w + 5 {
        let gap = cols - title_w - hint_w - 4;
        return pad_ansi(
            &format!("{title} {}{}{RESET} {hint} ", fg(palette.border), "─".repeat(gap)),
            cols,
        );
    }
    let fill = cols.saturating_sub(title_w + 2);
    pad_ansi(
        &format!("{title} {}{}{RESET}", fg(palette.border), "─".repeat(fill)),
        cols,
    )
}

/// 渲染单条透明内联补全候选（第 3 列基线对齐，无实心底块）。
fn format_completion_row(
    item: &CompletionItem,
    label_width: usize,
    cols: usize,
    selected: bool,
    palette: &PanelPalette,
) -> String {
    let show_cat = !item.category.is_empty() && cols >= label_width + 26;
    let cat_width = if show_cat { 8usize } else { 0usize };
    let desc_width = cols.saturating_sub(3 + label_width + cat_width);

    let label_clipped = clip_ansi(&item.label, label_width.saturating_sub(1));
    let label_padded = format!(
        "{label_clipped}{}",
        " ".repeat(label_width.saturating_sub(visible_width(&label_clipped)))
    );
    let cat_padded = if show_cat {
        let clipped = clip_ansi(item.category, cat_width.saturating_sub(1));
        format!(
            "{clipped}{}",
            " ".repeat(cat_width.saturating_sub(visible_width(&clipped)))
        )
    } else {
        String::new()
    };
    let detail = clip_ansi(&item.detail, desc_width);

    if selected {
        let cat_part = if show_cat {
            format!("{}{cat_padded}{RESET}", fg(palette.added))
        } else {
            String::new()
        };
        format!(
            " {}→{RESET} {BOLD}{}{label_padded}{RESET}{cat_part}{}{detail}{RESET}",
            fg(palette.accent),
            fg(palette.text),
            fg(palette.text),
        )
    } else {
        let cat_part = if show_cat {
            format!("{DIM}{}{cat_padded}{RESET}", fg(palette.subtle))
        } else {
            String::new()
        };
        format!(
            "   {DIM}{}{label_padded}{RESET}{cat_part}{DIM}{}{detail}{RESET}",
            fg(palette.muted),
            fg(palette.muted),
        )
    }
}

/// 【群组终端】【模型底栏】渲染对齐 `ReplChrome::footer_line` 的透明状态行：
/// 左侧展示 `group  上下文%/窗口  Leader模型  思考等级 [worker:模型]`，右侧展示操作反馈或工作区目录与 Git 分支。
fn render_model_status_footer(
    state: &mut GroupTuiState,
    cols: usize,
    screen_row: u16,
    palette: &PanelPalette,
) -> String {
    let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
    let inner = cols.saturating_sub(pad * 2).max(1);
    let leader = &state.model_info.leader;
    let mode_text = "group";
    let context_text = leader.context_status();

    // 1. 按可用列宽分级组装左侧模型基础信息（窄屏保留模型名，宽屏同时展示思考等级与异构 Worker 模型）
    let mut left_parts = vec![
        mode_text.to_string(),
        context_text.clone(),
        leader.short_model.clone(),
    ];
    if inner >= 54 {
        left_parts.push(leader.thinking.clone());
    }
    if !state.model_info.worker_shares_leader && inner >= 72 {
        left_parts.push(format!("worker:{}", state.model_info.worker.short_model));
    }
    let left_plain = left_parts.join("  ");

    let right_is_notice = state.notice.is_some();
    let right_plain = match state.notice.as_deref() {
        Some(text) => text.to_string(),
        None => state.model_info.right_workspace_plain(),
    };

    let (fitted_left, fitted_right, gap) = fit_status_segments(&left_plain, &right_plain, inner);

    // 2. 记录底栏模型与思考等级的可点击列区间（点击直接呼出 `/model` 模型选择器）
    let model_prefix = format!("{mode_text}  {context_text}  ");
    if fitted_left.starts_with(&model_prefix) {
        let model_start = pad + visible_width(&model_prefix);
        let rest = &fitted_left[model_prefix.len()..];
        let model_span = if !rest.is_empty() && !rest.ends_with("...") {
            let mut end_rel = visible_width(&leader.short_model);
            let thinking_token = format!("{}  {}", leader.short_model, leader.thinking);
            if rest.starts_with(&thinking_token) {
                end_rel = visible_width(&thinking_token);
            }
            if model_start + end_rel <= cols {
                Some((screen_row, model_start as u16, (model_start + end_rel) as u16))
            } else {
                None
            }
        } else {
            None
        };
        state.hit_map.footer_model_hit = model_span;
    }

    // 3. 按主 REPL 语义色对左、右段分别着色
    let left_colored = colorize_group_left_status(&fitted_left, leader.context_ratio, palette);
    let right_colored = if fitted_right.is_empty() {
        String::new()
    } else if right_is_notice {
        let tone = if state.notice_is_error {
            palette.removed
        } else {
            palette.accent
        };
        format!("{}{fitted_right}{RESET}", fg(tone))
    } else {
        colorize_right_status(&fitted_right)
    };

    let mut line = format!(
        "{}{left_colored}{}{right_colored}{}",
        " ".repeat(pad),
        " ".repeat(gap),
        " ".repeat(pad)
    );
    if let (Some((pcol, prow)), Some((hit_row, start, end))) =
        (state.pointer, state.hit_map.footer_model_hit)
    {
        if prow == hit_row && pcol >= start && pcol < end {
            line = crate::cli::repl_runtime::control_tint_cols(
                &line,
                usize::from(start),
                usize::from(end),
            );
        }
    }
    pad_ansi(&line, cols)
}

/// 按主 REPL `colorize_left_status` 规范为群组底栏左侧着色（mode / context / model / thinking / worker）。
fn colorize_group_left_status(left_text: &str, context_ratio: f32, palette: &PanelPalette) -> String {
    let parts: Vec<&str> = left_text.split("  ").collect();
    if parts.len() < 2 {
        return format!("{}{left_text}{RESET}", fg(palette.text));
    }
    let mode_text = parts[0];
    let context = parts[1];
    let context_color = if context_ratio >= 0.9 {
        palette.removed
    } else if context_ratio >= 0.7 {
        palette.warning
    } else {
        palette.added
    };
    let mut out = format!(
        "{}{mode_text}{RESET}  {}{context}{RESET}",
        fg(palette.accent),
        fg(context_color)
    );
    for (idx, part) in parts.iter().enumerate().skip(2) {
        if part.is_empty() {
            continue;
        }
        out.push_str("  ");
        let color = if idx == 4 || part.starts_with("worker:") {
            palette.muted
        } else {
            palette.text
        };
        out.push_str(&format!("{}{part}{RESET}", fg(color)));
    }
    out
}

/// 渲染最底部透明按键提示行（完全对齐 `composer_frame/key_hints.rs` 的键名/说明对比层次）。
fn render_key_hints_row(
    state: &GroupTuiState,
    completion_open: bool,
    cols: usize,
    palette: &PanelPalette,
) -> String {
    let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
    let budget = cols.saturating_sub(pad * 2);
    let hints: &[(&str, &str)] = if completion_open {
        &[
            ("↑↓", "选择"),
            ("Tab/Enter", "补全"),
            ("Esc", "关闭"),
        ]
    } else if !state.input.is_empty() {
        &[
            ("Enter", "发送"),
            ("Shift+Enter", "换行"),
            ("Esc", "清空"),
            ("Ctrl+W", "删词"),
        ]
    } else {
        &[
            ("/", "命令"),
            ("@", "成员"),
            ("Tab", "检视页"),
            ("Ctrl+R", "启Runner"),
            ("Ctrl+G", "接渠道"),
            ("?", "帮助"),
            ("Esc", "退出"),
        ]
    };

    let key_style = fg(palette.text);
    let label_style = fg(palette.muted);
    let sep = format!("{label_style} · {RESET}");
    let mut body = String::new();
    let mut used = 0usize;
    for (key, label) in hints {
        let plain_w = visible_width(key) + 1 + visible_width(label);
        let extra = if used == 0 { 0 } else { 3 };
        if used + extra + plain_w > budget {
            break;
        }
        if used > 0 {
            body.push_str(&sep);
        }
        used += extra + plain_w;
        body.push_str(&format!("{key_style}{key}{RESET} {label_style}{label}{RESET}"));
    }
    pad_ansi(&format!("{}{body}", " ".repeat(pad)), cols)
}

/// 按输入框正文列宽折行单个逻辑行（与 `repl_text::wrapped_end_position` 及 `styled_line::wrap_styled_line` 完全同口径）。
fn wrap_input_line(text: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    for ch in text.chars() {
        if ch == '\t' {
            let next = ((width / 8 + 1) * 8).min(cols.saturating_sub(1));
            current.push_str(&" ".repeat(next.saturating_sub(width)));
            width = next;
            continue;
        }
        let ch_w = char_terminal_width(ch);
        if ch_w > 0 && width + ch_w > cols {
            lines.push(current);
            current = String::new();
            width = 0;
        }
        current.push(ch);
        width = width.saturating_add(ch_w);
    }
    lines.push(current);
    // 恰好写满正文宽度时，为悬挂光标额外留出下一视觉行
    if width >= cols {
        lines.push(String::new());
    }
    lines
}

/// 截取包含光标行的可见输入视口（与 `composer_frame/input_viewport.rs` 一致）。
fn select_input_rows(lines: &[String], cursor_row: usize, limit: usize) -> (Vec<String>, u16) {
    let limit = limit.max(1);
    let cursor = cursor_row.min(lines.len().saturating_sub(1));
    let start = cursor
        .saturating_sub(limit / 2)
        .min(lines.len().saturating_sub(limit));
    (
        lines.iter().skip(start).take(limit).cloned().collect(),
        (cursor - start) as u16,
    )
}
