use super::super::render::{truncate, DIM_STYLE, FOCUS_STYLE, RESET};
use super::state::SubagentListState;
use crate::i18n::text as t;

/// 【终端】【子任务设置】渲染紧凑列表，模型和思考均在当前行可见。
///
/// 参数: `state` 为选择状态，`frame_rows` 为可用行数，`status` 为保存反馈
/// 返回: 固定高度的设置面板
pub(super) fn list(state: &SubagentListState, frame_rows: u16, status: &str) -> Vec<String> {
    let rows = usize::from(frame_rows);
    let content = content_rows(rows);
    let budget = content.len();
    let start = window_start(state, budget);
    let mut lines = vec![
        format!(
            "{FOCUS_STYLE}{}{RESET}",
            t("Subagent models & thinking", "子任务模型与思考")
        ),
        format!(
            "{DIM_STYLE}{}{RESET}",
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
    for (offset, target) in state.targets.iter().skip(start).take(budget).enumerate() {
        let selected = start + offset == state.index;
        let marker = if selected { "›" } else { " " };
        let style = if selected { FOCUS_STYLE } else { DIM_STYLE };
        let choice = &target.selection;
        let model = if choice.model.is_empty() {
            t("inherit", "沿用上层").to_string()
        } else {
            format!("{} / {}", choice.provider_id, choice.model)
        };
        let name = truncate(&target.name, 20);
        let gap = 22usize.saturating_sub(unicode_width::UnicodeWidthStr::width(name.as_str()));
        lines.push(format!(
            "{style}{marker} {name}{}{model}  ·  {}{RESET}",
            " ".repeat(gap),
            choice.thinking_level
        ));
    }
    while lines.len() < content.end {
        lines.push(String::new());
    }
    if rows >= 6 {
        lines.push(format!("{DIM_STYLE}{status}{RESET}"));
    }
    let width =
        crossterm::terminal::size().map_or(80, |(cols, _)| usize::from(cols.saturating_sub(1)));
    lines.push(controls(width).text);
    for line in &mut lines {
        *line = crate::render::ansi_clip::clip_ansi_to_width(line, width, "...");
    }
    lines.truncate(rows);
    lines
}

/// 【终端】【子任务窗口】参数为状态和行预算；返回可见列表起点，绘制和点击共用。
pub(super) fn window_start(state: &SubagentListState, budget: usize) -> usize {
    state
        .index
        .saturating_sub(budget / 2)
        .min(state.targets.len().saturating_sub(budget))
}

/// 【终端】【子任务返回】参数为可用列数；返回可点击返回按钮。
pub(super) fn controls(width: usize) -> crate::cli::mouse_menu::ButtonRow<super::ListTarget> {
    crate::cli::mouse_menu::ButtonRow::new(
        [(t("Back", "返回").into(), super::ListTarget::Back)],
        width,
    )
}

/// 【终端】【子任务高度】参数为可用行数；返回候选内容区，矮屏优先保留返回按钮。
pub(super) fn content_rows(rows: usize) -> std::ops::Range<usize> {
    if rows >= 6 {
        3..rows - 2
    } else {
        0..rows.saturating_sub(1)
    }
}
