use super::state::{PickerColumn, PickerState};
use crate::config::ProviderModelChoice;
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};
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
/// 帧固定行数：标题 2 + 页签 + 列标题 + 内容窗口 + 状态行 + 底栏
const FRAME_FIXED_ROWS: u16 = 2 + 1 + 1 + MODEL_VISIBLE_ROWS as u16 + 1 + 1;

pub(super) const RESET: &str = "\x1b[0m";

/// 模型选择器明暗自适应样式集。
#[derive(Clone)]
pub(super) struct PickerTheme {
    pub(super) focus: String,
    pub(super) dim: String,
}

impl PickerTheme {
    /// 【模型选择】【当前主题】从终端实时明暗调色板构造样式集。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 明暗自适应样式集
    pub(super) fn current() -> Self {
        Self::from_palette(PanelPalette::current())
    }

    /// 【模型选择】【调色板转换】根据给定面板调色板生成满足对比度的前景样式。
    ///
    /// 参数:
    /// - `palette`: 终端面板调色板
    ///
    /// 返回:
    /// - 样式集
    pub(super) fn from_palette(palette: PanelPalette) -> Self {
        Self {
            focus: format!("\x1b[1m{}", fg(palette.accent)),
            dim: fg(palette.muted),
        }
    }
}

/// 【CLI/TUI】【模型选择】返回选择器占用的固定终端行数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 预留给内联绘制的行数
pub(super) fn frame_row_count() -> u16 {
    FRAME_FIXED_ROWS
}

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
///
/// 参数:
/// - `state`: 选择状态
/// - `width`: 可用显示宽度
///
/// 返回:
/// - 固定行数的面板内容
#[cfg(test)]
pub(super) fn render_with_width(state: &PickerState, width: usize) -> Vec<String> {
    render_sized(state, width, FRAME_FIXED_ROWS as usize, true)
}

/// 【终端】【模型绘制】根据状态与宽高生成与鼠标命中共享布局的可见行。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `width`: 面板可用列宽
/// - `rows`: 面板可用行数
/// - `tasks`: 是否在底栏展示子任务入口
///
/// 返回:
/// - 可见 ANSI 行序列
pub(super) fn render_sized(
    state: &PickerState,
    width: usize,
    rows: usize,
    tasks: bool,
) -> Vec<String> {
    render_sized_with_palette(state, width, rows, tasks, PanelPalette::current())
}

/// 【终端】【指定主题模型绘制】按给定调色板渲染模型选择面板。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `width`: 面板可用列宽
/// - `rows`: 面板可用行数
/// - `tasks`: 是否在底栏展示子任务入口
/// - `palette`: 终端调色板
///
/// 返回:
/// - 可见 ANSI 行序列
pub(super) fn render_sized_with_palette(
    state: &PickerState,
    width: usize,
    rows: usize,
    tasks: bool,
    palette: PanelPalette,
) -> Vec<String> {
    let theme = PickerTheme::from_palette(palette);
    let layout = super::mouse::PickerLayout::new(state, width, rows);
    let widths = layout.widths;
    let mut lines = Vec::new();
    if rows >= 7 {
        lines.extend(header_lines(state, width, &theme, palette));
    }
    if rows > 1 {
        let tab_buttons = super::mouse::tabs(state, width);
        lines.push(super::hover_style::style_tabs_row(
            &tab_buttons,
            state.column(),
            state.hovered(),
            palette,
        ));
    }
    if rows >= 7 {
        lines.push(column_titles(state, widths, palette));
    }
    let (model_start, models) = state.model_window(layout.visible);
    // 1. 【模型选择】【固定帧高】内容区按布局可视行数填充，避免过滤后帧高抖动
    for index in 0..layout.visible {
        lines.push(row_line(state, models, model_start, index, &layout, palette));
    }
    let status = state
        .context_selection()
        .map(|context| context.status())
        .unwrap_or_default();
    if rows >= 7 {
        lines.push(format!("{}{}{RESET}", theme.dim, truncate(&status, width)));
    }
    // 2. 【模型选择】【底栏按钮】主操作保存强调着色，鼠标悬停任意按钮时高亮反馈
    let control_buttons = super::mouse::controls(width, tasks);
    lines.push(super::hover_style::style_action_buttons(
        &control_buttons,
        state.hovered(),
        Some(super::mouse::Target::Key(crossterm::event::KeyCode::Enter)),
        palette,
    ));
    lines.truncate(rows);
    lines
}

/// 【终端】【输入光标定位】计算过滤框或自定义上下文输入相对面板首行的光标坐标。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `width`: 面板可用列宽
/// - `rows`: 面板可用行数
///
/// 返回:
/// - `(列坐标, 面板相对行号)`，矮屏不展示输入行时返回空
pub(super) fn input_cursor_position(
    state: &PickerState,
    width: usize,
    rows: usize,
) -> Option<(u16, u16)> {
    if rows < 7 || width == 0 {
        return None;
    }
    let layout = super::mouse::PickerLayout::new(state, width, rows);
    // 1. 【模型选择】【上下文光标】聚焦上下文自定义行时将光标放在数值输入末尾
    if state.column() == PickerColumn::Context && layout.widths[2] > 0 {
        if let Some(ctx) = state.context_selection().filter(|c| c.is_custom_selected()) {
            let idx = ctx.index();
            if idx >= layout.starts[2] && idx < layout.starts[2] + layout.visible {
                let mut left = 0usize;
                for col_w in [layout.widths[0], layout.widths[1]] {
                    if col_w > 0 {
                        left += col_w + COLUMN_GAP;
                    }
                }
                let prefix = format!("› {}: ", t("Input", "输入"));
                let typed = UnicodeWidthStr::width(ctx.custom_input());
                let col = (left + UnicodeWidthStr::width(prefix.as_str()) + typed)
                    .min(width.saturating_sub(1));
                let row = layout.content_start + (idx - layout.starts[2]);
                return Some((col as u16, row as u16));
            }
        }
    }
    // 2. 【模型选择】【过滤框光标】默认停在过滤文本末尾以锚定输入法候选窗
    let prefix = format!("{}: ", t("Filter", "过滤"));
    let col = (UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(state.filter()))
        .min(width.saturating_sub(1));
    Some((col as u16, 1))
}

/// 【模型选择】【顶栏渲染】渲染紧凑标题、过滤文本和候选数量。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `width`: 可用列宽
/// - `theme`: 自适应样式集
/// - `palette`: 终端调色板
///
/// 返回:
/// - 标题与过滤提示两行
fn header_lines(
    state: &PickerState,
    width: usize,
    theme: &PickerTheme,
    palette: PanelPalette,
) -> Vec<String> {
    vec![
        format!(
            "{}{}{RESET}",
            theme.focus,
            truncate(t("Model selection", "模型选择"), width)
        ),
        super::hover_style::style_filter_line(state, width, palette),
    ]
}

/// 【终端】【模型选择】分配紧凑列宽，空间不足时保留聚焦列。
///
/// 参数:
/// - `state`: 选择状态
/// - `width`: 终端可用宽度
///
/// 返回:
/// - `[模型列宽, 思考列宽, 上下文列宽]`
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
///
/// 参数:
/// - `cells`: 各列内容
/// - `widths`: 各列宽度
///
/// 返回:
/// - 一行列内容
fn join_columns(cells: [String; 3], widths: [usize; 3]) -> String {
    cells
        .into_iter()
        .zip(widths)
        .filter_map(|(cell, width)| (width > 0).then_some(cell))
        .collect::<Vec<_>>()
        .join(&" ".repeat(COLUMN_GAP))
}

/// 【模型选择】【列标题行】渲染三列标题，聚焦列加亮，悬停列铺底提示。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `widths`: 模型、思考和上下文列宽
/// - `palette`: 终端调色板
///
/// 返回:
/// - 标题行文本
fn column_titles(state: &PickerState, widths: [usize; 3], palette: PanelPalette) -> String {
    let hovered = state.hovered();
    let model = super::hover_style::style_column_title(
        t("Provider / model", "供应商 / 模型"),
        widths[0],
        state.column() == PickerColumn::Model,
        hovered == Some(super::mouse::Target::Focus(PickerColumn::Model)),
        palette,
    );
    let thinking = super::hover_style::style_column_title(
        t("Thinking", "思考"),
        widths[1],
        state.column() == PickerColumn::Thinking,
        hovered == Some(super::mouse::Target::Focus(PickerColumn::Thinking)),
        palette,
    );
    let context = super::hover_style::style_column_title(
        t("Context limit", "上下文上限"),
        widths[2],
        state.column() == PickerColumn::Context,
        hovered == Some(super::mouse::Target::Focus(PickerColumn::Context)),
        palette,
    );
    join_columns([model, thinking, context], widths)
}

/// 【模型选择】【内容行】渲染一行可见列内容，独立计算三列焦点条与鼠标悬停反馈。
///
/// 参数:
/// - `state`: 当前选择状态
/// - `models`: 当前模型窗口
/// - `model_start`: 模型窗口起始下标
/// - `index`: 窗口内行下标
/// - `layout`: 当前共享列布局
/// - `palette`: 终端调色板
///
/// 返回:
/// - 定宽内容行字符串
fn row_line(
    state: &PickerState,
    models: &[ProviderModelChoice],
    model_start: usize,
    index: usize,
    layout: &super::mouse::PickerLayout,
    palette: PanelPalette,
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
    let model_idx = model_start + index;
    let level_index = layout.starts[1] + index;
    let context_index = layout.starts[2] + index;
    let hovered = state.hovered();
    let level = state.levels().get(level_index).copied();
    let model_cell = super::hover_style::style_item_cell(
        model_label.as_deref(),
        !state.models().is_empty() && model_idx == state.model_index(),
        model_focused,
        !state.models().is_empty()
            && hovered == Some(super::mouse::Target::Item(PickerColumn::Model, model_idx)),
        widths[0],
        palette,
    );
    let thinking_cell = super::hover_style::style_item_cell(
        level,
        level_index == state.level_index(),
        state.column() == PickerColumn::Thinking,
        hovered == Some(super::mouse::Target::Item(PickerColumn::Thinking, level_index)),
        widths[1],
        palette,
    );
    let context = state.context_selection();
    let context_label = context.and_then(|context| context.label(context_index));
    let context_cell = super::hover_style::style_item_cell(
        context_label.as_deref(),
        context.is_some_and(|context| context.index() == context_index),
        state.column() == PickerColumn::Context,
        hovered == Some(super::mouse::Target::Item(PickerColumn::Context, context_index)),
        widths[2],
        palette,
    );
    join_columns([model_cell, thinking_cell, context_cell], widths)
}

/// 【模型选择】【宽度截断】将纯文本截断到指定显示宽度。
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
    use crate::render::terminal_contrast::{contrast, MIN_TEXT_CONTRAST};

    fn choice(provider: &str, model: &str) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider.to_lowercase(),
            provider_name: provider.to_string(),
            model: model.to_string(),
        }
    }

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

    #[test]
    fn pads_the_shorter_column() {
        let picker = state();
        let lines = render(&picker);

        assert!(
            lines.iter().any(|line| line.contains("max")),
            "内容行数应覆盖较长的思考列: {lines:?}"
        );
    }

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

    #[test]
    fn omits_the_tui_logo() {
        let rendered = render(&state()).join("\n");

        assert!(!rendered.contains("███"));
        assert!(rendered.contains(t("Model selection", "模型选择")));
    }

    #[test]
    fn renders_all_provider_labels_and_filter() {
        let mut picker = state();
        picker.push_filter('d');
        let rendered = render(&picker).join("\n");

        assert!(rendered.contains("DeepSeek / deepseek-chat"));
        assert!(rendered.contains(t("Filter", "过滤")));
        assert!(rendered.contains("1/2"));
    }

    /// 【模型选择测试】【明暗自适应】验证浅色与深色终端下不再出现硬编码浅冰青且满足可读对比度。
    #[test]
    fn theme_adapts_for_light_and_dark_backgrounds() {
        for bg in [(16, 18, 22), (250, 250, 252), (194, 213, 231)] {
            let palette = PanelPalette::for_background(bg);
            let rendered =
                render_sized_with_palette(&state(), 100, FRAME_FIXED_ROWS as usize, true, palette)
                    .join("\n");
            assert!(!rendered.contains("190;246;255"));
            assert!(contrast(palette.accent, bg) >= MIN_TEXT_CONTRAST);
            assert!(contrast(palette.muted, bg) >= MIN_TEXT_CONTRAST);
        }
    }
}
