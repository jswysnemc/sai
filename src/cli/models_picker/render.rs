use super::state::{PickerColumn, PickerState};
use crate::config::ProviderModelChoice;
use crate::i18n::text as t;
use unicode_width::UnicodeWidthStr;

/// 模型列可用的最大展示宽度
const MODEL_COLUMN_WIDTH: usize = 46;
/// 思考等级列可用的最大展示宽度
const THINKING_COLUMN_WIDTH: usize = 12;
/// 上下文列可用的最大展示宽度
const CONTEXT_COLUMN_WIDTH: usize = 28;
/// 模型列最多展示的行数
const MODEL_VISIBLE_ROWS: usize = 12;
/// 两列之间的间隔
pub(super) const COLUMN_GAP: usize = 3;
/// 帧固定行数：标题 2 + 空行 + 列标题 + 内容窗口 + 空行 + 底栏
const FRAME_FIXED_ROWS: u16 = 2 + 1 + 1 + MODEL_VISIBLE_ROWS as u16 + 1 + 1;

/// 【CLI/TUI】【模型选择】返回选择器占用的固定终端行数。
///
/// 返回:
/// - 预留给内联绘制的行数
pub(super) fn frame_row_count() -> u16 {
    FRAME_FIXED_ROWS
}
/// 聚焦列的高亮样式
pub(super) const FOCUS_STYLE: &str = "\x1b[1m\x1b[38;2;190;246;255m";
/// 选中项样式
pub(super) const SELECTED_STYLE: &str = "\x1b[38;2;92;176;194m";
/// 弱化样式
pub(super) const DIM_STYLE: &str = "\x1b[2m";
pub(super) const RESET: &str = "\x1b[0m";

/// 【CLI】【模型选择】按终端宽度渲染模型、思考及可选上下文列。
///
/// 参数:
/// - `state`: 当前选择状态
///
/// 返回:
/// - 逐行 ANSI 文本
#[cfg(test)]
pub(super) fn render(state: &PickerState) -> Vec<String> {
    let width = crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(100);
    render_with_width(state, width.saturating_sub(1))
}

/// 【终端】【模型选择】使用固定宽度渲染，窄终端只展示当前聚焦列。
/// 参数: `state` 为选择状态，`width` 为可用显示宽度；返回: 固定行数的面板内容
#[cfg(test)]
pub(super) fn render_with_width(state: &PickerState, width: usize) -> Vec<String> {
    render_sized(state, width, FRAME_FIXED_ROWS as usize, true)
}

/// 【终端】【模型绘制】参数为状态、宽高及子任务开关；返回与鼠标共享布局的可见行。
pub(super) fn render_sized(
    state: &PickerState,
    width: usize,
    rows: usize,
    tasks: bool,
) -> Vec<String> {
    let layout = super::mouse::PickerLayout::new(state, width, rows);
    let widths = layout.widths;
    let mut lines = Vec::new();
    if rows >= 7 {
        lines.extend(header_lines(state, width));
    }
    if rows > 1 {
        lines.push(super::mouse::tabs(state, width).text);
    }
    if rows >= 7 {
        lines.push(column_titles(state, widths));
    }
    let (model_start, models) = state.model_window(layout.visible);
    // 内容区固定 MODEL_VISIBLE_ROWS 行，与 frame_row_count 对齐，避免帧高抖动
    for index in 0..layout.visible {
        lines.push(row_line(state, models, model_start, index, &layout));
    }
    let status = state
        .context_selection()
        .map(|context| context.status())
        .unwrap_or_default();
    if rows >= 7 {
        lines.push(format!("{DIM_STYLE}{}{RESET}", truncate(&status, width)));
    }
    lines.push(super::mouse::controls(width, tasks).text);
    lines.truncate(rows);
    lines
}

/// 渲染紧凑标题、过滤文本和候选数量。
///
/// 参数:
/// - `state`: 当前选择状态
///
/// 返回:
/// - 标题与过滤提示行
fn header_lines(state: &PickerState, width: usize) -> Vec<String> {
    let filter = if state.filter().is_empty() {
        t("type to filter", "输入内容过滤").to_string()
    } else {
        state.filter().to_string()
    };
    vec![
        format!(
            "{FOCUS_STYLE}{}{RESET}",
            truncate(t("Model selection", "模型选择"), width)
        ),
        format!(
            "{DIM_STYLE}{}{RESET}",
            truncate(
                &format!(
                    "{}: {filter}  ({}/{})",
                    t("Filter", "过滤"),
                    state.models().len(),
                    state.total_model_count()
                ),
                width
            )
        ),
    ]
}

/// 【终端】【模型选择】分配紧凑列宽，空间不足时保留聚焦列。
/// 参数: `state` 为选择状态，`width` 为终端可用宽度；返回: 模型、思考、上下文列宽
pub(super) fn column_widths(state: &PickerState, width: usize) -> [usize; 3] {
    let minimum = if state.context_enabled() { 70 } else { 45 };
    if width < minimum {
        return match state.column() {
            PickerColumn::Model => [width, 0, 0],
            PickerColumn::Thinking => [0, width, 0],
            PickerColumn::Context => [0, 0, width],
        };
    }
    let context = if state.context_enabled() {
        CONTEXT_COLUMN_WIDTH
    } else {
        0
    };
    let gaps = if state.context_enabled() { 2 } else { 1 } * COLUMN_GAP;
    [
        MODEL_COLUMN_WIDTH.min(width.saturating_sub(THINKING_COLUMN_WIDTH + context + gaps)),
        THINKING_COLUMN_WIDTH,
        context,
    ]
}

/// 【终端】【模型选择】拼接可见列并保留固定列间距。
/// 参数: `cells` 为各列内容，`widths` 为各列宽度；返回: 一行列内容
fn join_columns(cells: [String; 3], widths: [usize; 3]) -> String {
    cells
        .into_iter()
        .zip(widths)
        .filter_map(|(cell, width)| (width > 0).then_some(cell))
        .collect::<Vec<_>>()
        .join(&" ".repeat(COLUMN_GAP))
}

/// 渲染列标题，聚焦列加亮。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `widths`: 模型、思考和上下文列宽
///
/// 返回:
/// - 标题行
fn column_titles(state: &PickerState, widths: [usize; 3]) -> String {
    let model_focused = state.column() == PickerColumn::Model;
    let model = column_title(
        t("Provider / model", "供应商 / 模型"),
        widths[0],
        model_focused,
    );
    let thinking = column_title(
        t("Thinking", "思考"),
        widths[1],
        state.column() == PickerColumn::Thinking,
    );
    let context = column_title(
        t("Context limit", "上下文上限"),
        widths[2],
        state.column() == PickerColumn::Context,
    );
    join_columns([model, thinking, context], widths)
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
    let text = truncate(&format!("{marker}{label}"), width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
    format!("{style}{text}{RESET}{}", " ".repeat(padding))
}

/// 渲染一行可见列内容。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `models`: 当前模型窗口
/// - `model_start`: 模型窗口起始下标
/// - `index`: 窗口内行下标
/// - `widths`: 模型、思考和上下文列宽
///
/// 返回:
/// - 定宽内容行
fn row_line(
    state: &PickerState,
    models: &[ProviderModelChoice],
    model_start: usize,
    index: usize,
    layout: &super::mouse::PickerLayout,
) -> String {
    let widths = layout.widths;
    let model_focused = state.column() == PickerColumn::Model;
    let model_label = models
        .get(index)
        .map(|choice| {
            if choice.provider_id.is_empty() && choice.model.is_empty() {
                choice.provider_name.clone()
            } else {
                choice.label()
            }
        })
        .or_else(|| {
            (index == 0 && state.models().is_empty())
                .then(|| t("No matching models", "没有匹配的模型").to_string())
        });
    let level_index = layout.starts[1] + index;
    let context_index = layout.starts[2] + index;
    let level = state.levels().get(level_index).copied();
    let model_cell = item_cell(
        model_label.as_deref(),
        !state.models().is_empty() && model_start + index == state.model_index(),
        model_focused,
        widths[0],
    );
    let thinking_cell = item_cell(
        level,
        level_index == state.level_index(),
        state.column() == PickerColumn::Thinking,
        widths[1],
    );
    let context = state.context_selection();
    let context_label = context.and_then(|context| context.label(context_index));
    let context_cell = item_cell(
        context_label.as_deref(),
        context.is_some_and(|context| context.index() == context_index),
        state.column() == PickerColumn::Context,
        widths[2],
    );
    join_columns([model_cell, thinking_cell, context_cell], widths)
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

/// 将文本截断到指定显示宽度。
///
/// 参数:
/// - `text`: 原始文本
/// - `width`: 最大宽度
///
/// 返回:
/// - 不超过最大宽度的文本
pub(super) fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
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
        assert!(rendered.contains(t("Model selection", "模型选择")));
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
        assert!(rendered.contains(t("Filter", "过滤")));
        assert!(rendered.contains("1/2"));
    }
}
