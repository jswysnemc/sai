use super::super::render::{truncate, PickerTheme, RESET};
use super::state::SubagentListState;
use crate::i18n::text as t;
use crate::render::panel_palette::PanelPalette;
use unicode_width::UnicodeWidthStr;

/// 【终端】【子任务设置】渲染紧凑列表，采用左右对齐保证 80 列下右侧思考等级始终完整可见。
///
/// 参数:
/// - `state`: 子任务列表状态
/// - `width`: 面板可用列宽
/// - `frame_rows`: 可用行数
/// - `status`: 保存结果状态文案
///
/// 返回:
/// - 固定高度的设置面板行列表
pub(super) fn list(
    state: &SubagentListState,
    width: usize,
    frame_rows: u16,
    status: &str,
) -> Vec<String> {
    list_with_palette(state, width, frame_rows, status, PanelPalette::current())
}

/// 【终端】【指定调色板子任务绘制】按给定调色板渲染带焦点与鼠标悬停高亮的子任务列表。
///
/// 参数:
/// - `state`: 子任务列表状态
/// - `width`: 面板可用列宽
/// - `frame_rows`: 可用行数
/// - `status`: 保存结果状态文案
/// - `palette`: 终端面板调色板
///
/// 返回:
/// - 固定高度的设置面板行列表
pub(super) fn list_with_palette(
    state: &SubagentListState,
    width: usize,
    frame_rows: u16,
    status: &str,
    palette: PanelPalette,
) -> Vec<String> {
    let theme = PickerTheme::from_palette(palette);
    let rows = usize::from(frame_rows);
    let width = width.max(1);
    let content = content_rows(rows);
    let budget = content.len();
    let start = window_start(state, budget);
    let mut lines = vec![
        format!(
            "{}{}{RESET}",
            theme.focus,
            t("Subagent models & thinking", "子任务模型与思考")
        ),
        format!(
            "{}{}{RESET}",
            theme.dim,
            t(
                "Shared defaults, with optional overrides by task type",
                "共享默认值，可按任务类型单独设置"
            )
        ),
        String::new(),
    ];
    if rows < 6 {
        lines.clear();
    }
    // 1. 【子任务设置】【左右对齐排版】优先预留右侧思考等级列宽，并渲染焦点或悬停行底色
    for (offset, target) in state.targets.iter().skip(start).take(budget).enumerate() {
        let index = start + offset;
        let selected = index == state.index;
        let hovered = state.hovered() == Some(super::ListTarget::Item(index));
        let marker = if selected { "›" } else { " " };
        let style = super::super::hover_style::subagent_row_style(selected, hovered, palette);
        let choice = &target.selection;
        let model = if choice.model.is_empty() {
            t("inherit", "沿用上层").to_string()
        } else {
            format!("{} / {}", choice.provider_id, choice.model)
        };
        let thinking = if choice.thinking_level.trim().is_empty() {
            "auto"
        } else {
            choice.thinking_level.trim()
        };
        lines.push(format_subagent_row(
            &style,
            marker,
            &target.name,
            &model,
            thinking,
            width,
        ));
    }
    while lines.len() < content.end {
        lines.push(String::new());
    }
    if rows >= 6 {
        lines.push(format!("{}{status}{RESET}", theme.dim));
    }
    // 2. 【子任务设置】【底栏高亮】渲染返回按钮与快捷键说明，鼠标悬停返回按钮时高亮
    let control_buttons = controls(width);
    lines.push(super::super::hover_style::style_action_buttons(
        &control_buttons,
        state.hovered(),
        None,
        palette,
    ));
    for line in &mut lines {
        *line = crate::render::ansi_clip::clip_ansi_to_width(line, width, "…");
    }
    lines.truncate(rows);
    lines
}

/// 【子任务设置】【单行对齐】将名称、模型与右侧思考等级排版为不超过列宽的左右对齐行。
///
/// 参数:
/// - `style`: 行前景与背景样式
/// - `marker`: 左侧焦点箭头或空格
/// - `name`: 任务类型名称
/// - `model`: 供应商与模型文本
/// - `thinking`: 思考等级文本
/// - `width`: 可用总列宽
///
/// 返回:
/// - 带样式的单行文本
pub(super) fn format_subagent_row(
    style: &str,
    marker: &str,
    name: &str,
    model: &str,
    thinking: &str,
    width: usize,
) -> String {
    let right = format!("  ·  {thinking}");
    let right_w = UnicodeWidthStr::width(right.as_str());
    if width <= right_w + 10 {
        let compact_right = format!(" · {thinking}");
        let compact_w = UnicodeWidthStr::width(compact_right.as_str());
        if width <= compact_w + 4 {
            let text = truncate(&format!("{marker} {name}{compact_right}"), width);
            let pad = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
            return format!("{style}{text}{}{RESET}", " ".repeat(pad));
        }
        let left_budget = width.saturating_sub(compact_w);
        let left = truncate(&format!("{marker} {name}"), left_budget);
        let pad = left_budget.saturating_sub(UnicodeWidthStr::width(left.as_str()));
        return format!("{style}{left}{}{compact_right}{RESET}", " ".repeat(pad));
    }
    let name_col = 20.min((width.saturating_sub(right_w + 6)) / 2).max(8);
    let clipped_name = truncate(name, name_col.saturating_sub(2));
    let name_gap = name_col.saturating_sub(UnicodeWidthStr::width(clipped_name.as_str()));
    let left_prefix = format!("{marker} {clipped_name}{}", " ".repeat(name_gap));
    let prefix_w = UnicodeWidthStr::width(left_prefix.as_str());
    let model_budget = width.saturating_sub(prefix_w + right_w);
    let clipped_model = truncate(model, model_budget);
    let fill = model_budget.saturating_sub(UnicodeWidthStr::width(clipped_model.as_str()));
    format!(
        "{style}{left_prefix}{clipped_model}{}{right}{RESET}",
        " ".repeat(fill)
    )
}

/// 【终端】【子任务窗口】计算可见列表起点，供绘制和点击命中共享。
///
/// 参数:
/// - `state`: 列表状态
/// - `budget`: 可见行预算
///
/// 返回:
/// - 窗口起始下标
pub(super) fn window_start(state: &SubagentListState, budget: usize) -> usize {
    state
        .index
        .saturating_sub(budget / 2)
        .min(state.targets.len().saturating_sub(budget))
}

/// 【终端】【子任务返回】构造底栏返回按钮及快捷键说明。
///
/// 参数:
/// - `width`: 可用列数
///
/// 返回:
/// - 可点击的按钮行
pub(super) fn controls(width: usize) -> crate::cli::mouse_menu::ButtonRow<super::ListTarget> {
    crate::cli::mouse_menu::ButtonRow::new(
        [(t("Back", "返回").into(), super::ListTarget::Back)],
        width,
    )
    .with_hint(
        t(
            "Enter edit · Esc back · ↑↓ choose",
            "Enter 编辑 · Esc 返回 · ↑↓ 选择",
        ),
        width,
    )
}

/// 【终端】【子任务高度】返回列表候选内容行区间，矮屏优先保留返回按钮。
///
/// 参数:
/// - `rows`: 面板总行数
///
/// 返回:
/// - 内容行范围
pub(super) fn content_rows(rows: usize) -> std::ops::Range<usize> {
    if rows >= 6 {
        3..rows - 2
    } else {
        0..rows.saturating_sub(1)
    }
}
