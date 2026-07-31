use super::state::{PickerColumn, PickerState};
use crate::config::ProviderModelChoice;
use crate::i18n::text as t;
use unicode_width::UnicodeWidthStr;

/// 模型列可用的最大展示宽度
const MODEL_COLUMN_WIDTH: usize = 46;
/// 思考等级列可用的最大展示宽度
const THINKING_COLUMN_WIDTH: usize = 12;
/// 模型列最多展示的行数
const MODEL_VISIBLE_ROWS: usize = 12;
/// 两列之间的间隔
const COLUMN_GAP: usize = 3;
/// 聚焦列的高亮样式
const FOCUS_STYLE: &str = "\x1b[1m\x1b[38;2;190;246;255m";
/// 选中项样式
const SELECTED_STYLE: &str = "\x1b[38;2;92;176;194m";
/// 弱化样式
const DIM_STYLE: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// 【CLI】【模型选择】渲染无品牌 Logo 的双列选择界面。
///
/// 参数:
/// - `state`: 当前选择状态
///
/// 返回:
/// - 逐行 ANSI 文本
pub(super) fn render(state: &PickerState) -> Vec<String> {
    let mut lines = header_lines(state);
    lines.push(String::new());
    lines.push(column_titles(state));
    let (model_start, models) = state.model_window(MODEL_VISIBLE_ROWS);
    let rows = models.len().max(state.levels().len());
    for index in 0..rows {
        lines.push(row_line(state, models, model_start, index));
    }
    lines.push(String::new());
    lines.push(footer_line());
    lines
}

/// 渲染紧凑标题、过滤文本和候选数量。
///
/// 参数:
/// - `state`: 当前选择状态
///
/// 返回:
/// - 标题与过滤提示行
fn header_lines(state: &PickerState) -> Vec<String> {
    let filter = if state.filter().is_empty() {
        t("type to filter", "输入内容过滤").to_string()
    } else {
        state.filter().to_string()
    };
    vec![
        format!("{FOCUS_STYLE}{}{RESET}", t("Model selection", "模型选择")),
        format!(
            "{DIM_STYLE}{}:{RESET} {filter}  {DIM_STYLE}({}/{}){RESET}",
            t("Filter", "过滤"),
            state.models().len(),
            state.total_model_count()
        ),
    ]
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
    let model = column_title(
        t("Provider / model", "供应商 / 模型"),
        MODEL_COLUMN_WIDTH,
        model_focused,
    );
    let thinking = column_title(t("Thinking", "思考"), THINKING_COLUMN_WIDTH, !model_focused);
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
/// - `models`: 当前模型窗口
/// - `model_start`: 模型窗口起始下标
/// - `index`: 窗口内行下标
///
/// 返回:
/// - 定宽内容行
fn row_line(
    state: &PickerState,
    models: &[ProviderModelChoice],
    model_start: usize,
    index: usize,
) -> String {
    let model_focused = state.column() == PickerColumn::Model;
    let model_label = models
        .get(index)
        .map(ProviderModelChoice::label)
        .or_else(|| {
            (index == 0 && state.models().is_empty())
                .then(|| t("No matching models", "没有匹配的模型").to_string())
        });
    let level = state.levels().get(index).copied();
    let model_cell = item_cell(
        model_label.as_deref(),
        !state.models().is_empty() && model_start + index == state.model_index(),
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
            "Type to filter · Backspace edit · Ctrl+U clear · ↑/↓ choose · ←/→ switch · Enter save · Esc cancel",
            "输入过滤 · Backspace 删除 · Ctrl+U 清空 · ↑/↓ 选择 · ←/→ 切换 · Enter 保存 · Esc 取消",
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

    /// 构造供应商模型候选。
    ///
    /// 参数:
    /// - `provider`: 供应商展示名
    /// - `model`: 模型名称
    ///
    /// 返回:
    /// - 供应商模型候选
    fn choice(provider: &str, model: &str) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider.to_lowercase(),
            provider_name: provider.to_string(),
            model: model.to_string(),
        }
    }

    /// 构造样例选择状态。
    ///
    /// 返回:
    /// - 选择状态
    fn state() -> PickerState {
        PickerState::new(
            vec![
                choice("OpenAI", "gpt-5"),
                choice("DeepSeek", "deepseek-chat"),
            ],
            vec!["auto", "high", "max"],
            "openai",
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
        let model_focused = render(&picker).join("\n");
        picker.focus_thinking();
        let thinking_focused = render(&picker).join("\n");

        assert_ne!(model_focused, thinking_focused);
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
        let lines = render(&picker);

        assert!(
            lines.iter().any(|line| line.contains("max")),
            "内容行数应覆盖较长的思考列: {lines:?}"
        );
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
        let picker = PickerState::new(
            vec![choice("OpenAI", &long)],
            vec!["auto"],
            "openai",
            &long,
            "auto",
        );
        let lines = render(&picker);

        assert!(
            lines.iter().any(|line| line.contains('…')),
            "超长名称应被截断"
        );
    }

    /// 【CLI】【模型选择】验证 models 命令使用紧凑标题而不展示 TUI Logo。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn omits_the_tui_logo() {
        let rendered = render(&state()).join("\n");

        assert!(!rendered.contains("███"));
        assert!(rendered.contains("模型"));
    }

    /// 【CLI】【模型选择】验证模型标签包含供应商并展示过滤文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn renders_all_provider_labels_and_filter() {
        let mut picker = state();
        picker.push_filter('d');
        let rendered = render(&picker).join("\n");

        assert!(rendered.contains("DeepSeek / deepseek-chat"));
        assert!(rendered.contains("过滤"));
        assert!(rendered.contains("1/2"));
    }
}
