use super::super::render::{truncate, DIM_STYLE, FOCUS_STYLE, RESET};
use super::state::SubagentListState;
use crate::i18n::text as t;

/// 【终端】【子任务设置】渲染紧凑列表，模型和思考均在当前行可见。
///
/// 参数: `state` 为选择状态，`frame_rows` 为可用行数，`status` 为保存反馈
/// 返回: 固定高度的设置面板
pub(super) fn list(state: &SubagentListState, frame_rows: u16, status: &str) -> Vec<String> {
    let rows = usize::from(frame_rows);
    let budget = rows.saturating_sub(5).max(1);
    let start = state
        .index
        .saturating_sub(budget / 2)
        .min(state.targets.len().saturating_sub(budget));
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
    while lines.len() < rows.saturating_sub(2) {
        lines.push(String::new());
    }
    lines.push(format!("{DIM_STYLE}{status}{RESET}"));
    lines.push(format!(
        "{DIM_STYLE}{}{RESET}",
        t(
            "↑/↓ choose · Enter configure · Esc back",
            "↑/↓ 选择 · Enter 配置 · Esc 返回"
        )
    ));
    lines.truncate(rows);
    lines
}
