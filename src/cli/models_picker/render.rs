use super::state::{PickerColumn, PickerState};
use crate::i18n::text as t;
use crate::render::brand_logo::logo_lines;
use unicode_width::UnicodeWidthStr;

/// 模型列可用的最大展示宽度
const MODEL_COLUMN_WIDTH: usize = 32;
/// 思考等级列可用的最大展示宽度
const THINKING_COLUMN_WIDTH: usize = 12;
/// 两列之间的间隔
const COLUMN_GAP: usize = 3;
/// 聚焦列的高亮样式
const FOCUS_STYLE: &str = "\x1b[1m\x1b[38;2;190;246;255m";
/// 选中项样式
const SELECTED_STYLE: &str = "\x1b[38;2;92;176;194m";
/// 弱化样式
const DIM_STYLE: &str = "\x1b[2m";
/// 品牌色
const BRAND_STYLE: &str = "\x1b[38;2;58;114;100m";
const RESET: &str = "\x1b[0m";

/// 【CLI】【模型选择】渲染双列选择界面。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `provider`: 当前供应商展示名
///
/// 返回:
/// - 逐行 ANSI 文本
pub(super) fn render(state: &PickerState, provider: &str) -> Vec<String> {
    let mut lines = header_lines(provider);
    lines.push(String::new());
    lines.push(column_titles(state));
    let rows = state.models().len().max(state.levels().len());
    for index in 0..rows {
        lines.push(row_line(state, index));
    }
    lines.push(String::new());
    lines.push(footer_line());
    lines
}

/// 渲染顶部品牌标志与供应商说明。
///
/// 参数:
/// - `provider`: 当前供应商展示名
///
/// 返回:
/// - 标志与说明行
fn header_lines(provider: &str) -> Vec<String> {
    let logo = logo_lines(BRAND_STYLE);
    let caption = [
        String::new(),
        format!(
            "{DIM_STYLE}{}{RESET} {provider}",
            t("provider", "供应商")
        ),
        format!(
            "{DIM_STYLE}{}{RESET}",
            t("model and thinking level", "模型与思考等级")
        ),
        String::new(),
        String::new(),
    ];
    logo.into_iter()
        .zip(caption)
        .map(|(row, text)| {
            if text.is_empty() {
                row
            } else {
                format!("{row}  {text}")
            }
        })
        .collect()
}

/// 渲染两列标题，聚焦列加亮。
///
/// 参数:
/// - `state`: 当前选择状态
///
/// 返回:
/// - 标题行
fn column_titles(state: &PickerState) -> String {
    let model_focused = state.column() == PickerColumn::Model;
    let model = column_title(t("Model", "模型"), MODEL_COLUMN_WIDTH, model_focused);
    let thinking = column_title(
        t("Thinking", "思考"),
        THINKING_COLUMN_WIDTH,
        !model_focused,
    );
    format!("{model}{}{thinking}", " ".repeat(COLUMN_GAP))
}

/// 渲染单个列标题。
///
/// 参数:
/// - `label`: 标题文本
/// - `width`: 列宽
/// - `focused`: 是否为聚焦列
///
/// 返回:
/// - 定宽标题
fn column_title(label: &str, width: usize, focused: bool) -> String {
    let marker = if focused { "▸ " } else { "  " };
    let style = if focused { FOCUS_STYLE } else { DIM_STYLE };
    let text = format!("{marker}{label}");
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
    format!("{style}{text}{RESET}{}", " ".repeat(padding))
}

/// 渲染一行两列内容。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `index`: 行下标
///
/// 返回:
/// - 定宽内容行
fn row_line(state: &PickerState, index: usize) -> String {
    let model_focused = state.column() == PickerColumn::Model;
    let model = state.models().get(index).map(String::as_str);
    let level = state.levels().get(index).copied();
    let model_cell = item_cell(
        model,
        index == state.model_index(),
        model_focused,
        MODEL_COLUMN_WIDTH,
    );
    let thinking_cell = item_cell(
        level,
        index == state.level_index(),
        !model_focused,
        THINKING_COLUMN_WIDTH,
    );
    format!("{model_cell}{}{thinking_cell}", " ".repeat(COLUMN_GAP))
}

/// 渲染单个候选项。
///
/// 参数:
/// - `label`: 候选项文本，None 表示该列本行为空
/// - `selected`: 是否为该列当前选中项
/// - `focused`: 所在列是否聚焦
/// - `width`: 列宽
///
/// 返回:
/// - 定宽候选项
fn item_cell(label: Option<&str>, selected: bool, focused: bool, width: usize) -> String {
    let Some(label) = label else {
        return " ".repeat(width);
    };
    // 聚焦列的选中项用箭头，未聚焦列只保留圆点，避免两列同时抢注意力
    let marker = match (selected, focused) {
        (true, true) => "› ",
        (true, false) => "· ",
        _ => "  ",
    };
    let style = match (selected, focused) {
        (true, true) => FOCUS_STYLE,
        (true, false) => SELECTED_STYLE,
        _ => DIM_STYLE,
    };
    let text = truncate(&format!("{marker}{label}"), width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
    format!("{style}{text}{RESET}{}", " ".repeat(padding))
}

/// 渲染操作提示。
///
/// 返回:
/// - 提示行
fn footer_line() -> String {
    format!(
        "{DIM_STYLE}{}{RESET}",
        t(
            "↑/↓ choose · ←/→ switch column · Enter save · Esc cancel",
            "↑/↓ 选择 · ←/→ 切换列 · Enter 保存 · Esc 取消",
        )
    )
}

/// 将文本截断到指定显示宽度。
///
/// 参数:
/// - `text`: 原始文本
/// - `width`: 最大宽度
///
/// 返回:
/// - 不超过最大宽度的文本
fn truncate(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let char_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > width.saturating_sub(1) {
            break;
        }
        output.push(ch);
        used += char_width;
    }
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造样例选择状态。
    ///
    /// 返回:
    /// - 选择状态
    fn state() -> PickerState {
        PickerState::new(
            vec!["gpt-5".to_string(), "gpt-5-mini".to_string()],
            vec!["auto", "high", "max"],
            "gpt-5",
            "high",
        )
    }

    /// 【CLI】【模型选择】验证聚焦列切换时高亮随之移动。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn focus_marker_follows_the_active_column() {
        let mut picker = state();
        let model_focused = render(&picker, "openai").join("\n");
        picker.focus_thinking();
        let thinking_focused = render(&picker, "openai").join("\n");

        assert_ne!(model_focused, thinking_focused);
        // 聚焦列的选中项用 › 标记，同一时刻只有一列带它
        assert_eq!(model_focused.matches('›').count(), 1);
        assert_eq!(thinking_focused.matches('›').count(), 1);
    }

    /// 【CLI】【模型选择】验证两列行数不等时较短列补空而不越界。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn pads_the_shorter_column() {
        let picker = state();
        let lines = render(&picker, "openai");

        // 模型两项、思考三项，内容行应取较大值
        let body = lines
            .iter()
            .filter(|line| line.contains("gpt-5") || line.contains("auto") || line.contains("max"))
            .count();
        assert!(body >= 3, "内容行数应覆盖较长的一列: {lines:?}");
    }

    /// 【CLI】【模型选择】验证超长模型名被截断而不撑破列宽。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn truncates_overlong_model_names() {
        let long = "a".repeat(80);
        let picker = PickerState::new(vec![long.clone()], vec!["auto"], &long, "auto");
        let lines = render(&picker, "openai");

        assert!(
            lines.iter().any(|line| line.contains('…')),
            "超长名称应被截断"
        );
    }
}
