//! 全屏会话视图的逐行组装：浮动标题、正文、概览轨道、悬停预览与滚动条。
//!
//! 纯函数：只根据状态与分区生成每一行的 ANSI 文本，由调用方统一做差异绘制。

use super::layout::FullscreenLayout;
use super::overview::{mark_for_anchor, rail_marks};
use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::i18n::text as t;
use crate::render::panel_palette::{bg, fg, PanelPalette, Rgb};
use crate::render::terminal_palette::terminal_palette;

/// 暗色终端顶栏默认背景（与输入框面板保持同档深灰）。
const DARK_HEADER_BG: &str = "\x1b[48;5;235m";
const RESET: &str = "\x1b[0m";
/// 悬停预览最大宽度。
const PREVIEW_MAX_WIDTH: usize = 48;

/// 全屏画布动态明暗配色。
struct CanvasTheme {
    header_bg: String,
    header_index: String,
    header_text: String,
    header_copied: String,
    rail_idle: String,
    rail_active: String,
    rail_hover: String,
    track: String,
    thumb: String,
    preview_bg: String,
    preview_text: String,
}

/// 【全屏画布】【亮度判定】判断终端背景是否为浅色，与 PanelPalette 亮度阈值一致。
/// 参数: `background` 为背景 RGB；返回: 浅色背景时返回 true。
pub(super) fn is_light_bg((r, g, b): Rgb) -> bool {
    u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000
}

impl CanvasTheme {
    /// 【全屏画布】【当前主题】按缓存的终端背景构建全屏顶栏、轨道与滚动条配色。
    /// 参数: 无；返回: 明暗自适应画布主题。
    fn current() -> Self {
        Self::for_background(terminal_palette().background)
    }

    /// 【全屏画布】【指定背景主题】根据给定背景 RGB 生成画布色彩，消除浅色黑条与滚动条明暗反转。
    /// 参数: `background` 为终端背景 RGB；返回: 画布主题。
    fn for_background(background: Rgb) -> Self {
        let palette = PanelPalette::for_background(background);
        let header_bg = if is_light_bg(background) {
            bg(palette.hover_bg)
        } else {
            DARK_HEADER_BG.to_string()
        };
        Self {
            header_bg,
            header_index: fg(palette.accent),
            header_text: fg(palette.text),
            header_copied: format!("\x1b[1m{}", fg(palette.accent)),
            rail_idle: fg(palette.border),
            rail_active: fg(palette.accent),
            rail_hover: fg(palette.text),
            track: fg(palette.border),
            thumb: fg(palette.muted),
            preview_bg: bg(palette.focus_bg),
            preview_text: fg(palette.text),
        }
    }
}

/// 组装结果：屏幕行与可点击的“回到底部”按钮区域。
pub(super) struct PaintedRows {
    /// 从第 0 行到正文末行的屏幕行（不含输入框）
    pub(super) rows: Vec<String>,
    /// 输入框正上方“回到底部”按钮所在列范围 [start, end)
    pub(super) bottom_button: Option<(u16, u16)>,
}

/// 【全屏画布】【整屏组装】组装标题与正文区的全部屏幕行。
/// 参数: `state` 为全屏状态，`layout` 为屏幕分区；返回: 逐行 ANSI 文本与按钮点击区域。
pub(super) fn compose(state: &FullscreenState, layout: &FullscreenLayout) -> PaintedRows {
    let theme = CanvasTheme::current();
    let header = header_row_with_theme(state, layout, &theme).0;
    let mut rows = vec![header];
    let body_height = usize::from(layout.body_height);
    let content_width = usize::from(layout.content_width);
    // 1. 【全屏画布】【右侧辅助列】计算概览标记与按需显示的滚动条滑块
    let marks = rail_marks(state.document.anchors.len(), body_height);
    let active = state
        .current_anchor()
        .and_then(|anchor| mark_for_anchor(&marks, anchor));
    let hovered = state
        .hover
        .filter(|_| layout.rail_col.is_some())
        .and_then(|anchor| mark_for_anchor(&marks, anchor));
    let thumb = (state.dragging
        || (state.pointer_col == Some(layout.scrollbar_col) && state.pointer_row.is_some()))
    .then(|| scroll_thumb(state, body_height))
    .flatten();
    // 2. 【全屏画布】【悬停卡片】悬停预览卡片停在被悬停标记的同一行
    let preview = hovered.and_then(|index| {
        let anchor = state.document.anchors.get(marks[index].anchor)?;
        Some((
            marks[index].row,
            preview_text(marks[index], anchor.summary.as_str(), state),
        ))
    });
    // 3. 【全屏画布】【窗口投影】跨行公式图片按窗口边界裁剪，滚动时保留固定段落标题
    let mut window = super::image_window::window_lines(
        &state.document.lines,
        state.scroll,
        body_height,
        crate::render::terminal_image::kitty_cell_pixel_height(),
    );
    if body_height > 1 {
        if let Some(span) = state.sticky_paragraph() {
            window[0] = state.document.lines[span.control].as_str().to_string();
        }
    }
    let mut bottom_button = None;
    // 4. 【全屏画布】【标题悬停】只提亮命中段落的控制行，正文保留原有语法颜色
    let hovered_span = state
        .hovered_paragraph()
        .filter(|_| !super::bottom_button::is_hovered(state, content_width, body_height));
    for (row, line) in window.iter().enumerate() {
        let document_row = state.body_document_row(row);
        let painted_line = super::block_scrollbar::paint_line(state, document_row, line);
        let line = painted_line.as_str();
        let body = match &preview {
            Some((preview_row, text)) if *preview_row == row => {
                overlay_preview(line, text, content_width, &theme)
            }
            _ => fit(line, content_width),
        };
        let body = match hovered_span {
            Some(span)
                if document_row == span.control
                    && !matches!(&preview, Some((preview_row, _)) if *preview_row == row) =>
            {
                if document_row == span.control && state.pointer_row == Some(row) {
                    super::hover::tint_line(&super::guide_button::render(&body, span.expanded))
                } else {
                    super::hover::tint_line(&body)
                }
            }
            _ => body,
        };
        let body = if state.hovered_activity_footer().is_some() && state.pointer_row == Some(row) {
            super::hover::tint_line(&super::guide_button::render(&body, false))
        } else {
            body
        };
        let body = super::block_scrollbar::preserve_gutter_color(&body, window[row].as_str());
        // 5. 【全屏画布】【拖选反色】拖动选区以反色标出
        let body = match state
            .selection
            .and_then(|selection| selection.cols_on_row(document_row, content_width))
        {
            Some((from, to)) => super::selection::highlight_cols(&body, from, to),
            None => body,
        };
        // 6. 【全屏画布】【底部胶囊】离开底部时在紧贴输入框的正文末行叠加回到底部胶囊
        let is_last = row + 1 == body_height;
        let hovered_here = matches!(&preview, Some((preview_row, _)) if *preview_row == row);
        let body = if is_last
            && !hovered_here
            && !super::bottom_button::conflicts_with_activity_footer(state, body_height)
        {
            let (line, cols) = super::bottom_button::overlay_at_row(
                &body,
                state,
                content_width,
                body_height,
                state.pointer_col,
            );
            bottom_button = cols;
            line
        } else {
            body
        };
        let mut output = body;
        if layout.rail_col.is_some() {
            let mark = marks.iter().position(|mark| mark.row == row);
            output.push(' ');
            output.push_str(&rail_glyph(mark, active, hovered, &theme));
        }
        output.push_str(&scrollbar_glyph(row, thumb, &theme));
        if layout.sidebar.is_some() {
            output.push(' ');
        }
        rows.push(output);
    }
    super::message_picker_view::overlay(state, layout, &mut rows);
    PaintedRows {
        rows,
        bottom_button,
    }
}

/// 【消息导航】【标题命中】使用实际绘制的左侧标题宽度，右侧位置指示与空白不响应点击。
/// 参数: state 为全屏状态，layout 为分区，column/row 为坐标；返回: 是否命中消息入口。
pub(super) fn header_target_at(
    state: &FullscreenState,
    layout: &FullscreenLayout,
    column: u16,
    row: u16,
) -> bool {
    !state.document.anchors.is_empty()
        && row < layout.body_top
        && usize::from(column)
            < crate::render::tui_presentation::with_locale(|| {
                header_row_with_theme(state, layout, &CanvasTheme::current()).1
            })
}

/// 【消息导航】【标题布局】同步生成自适应配色的标题文字和入口宽度。
/// 参数: state 为全屏状态，layout 为分区，theme 为画布主题；返回: ANSI 标题行与可点击列数。
fn header_row_with_theme(
    state: &FullscreenState,
    layout: &FullscreenLayout,
    theme: &CanvasTheme,
) -> (String, usize) {
    let cols = super::header_controls::title_width(layout);
    let total = state.document.anchors.len();
    let header_bg = &theme.header_bg;
    let header_index = &theme.header_index;
    let header_text = &theme.header_text;
    let header_copied = &theme.header_copied;
    // 1. 【消息导航】【左侧摘要】显示当前所在用户消息序号与摘要
    let left = match state.current_anchor() {
        Some(index) => format!(
            "{header_index}● {}/{total}{RESET}{header_bg} {header_text}{}",
            index + 1,
            state.document.anchors[index].summary
        ),
        None if total > 0 => format!(
            "{header_index}● 0/{total}{RESET}{header_bg} {header_text}{}",
            t("Start of conversation", "会话开头")
        ),
        None => format!("{header_text}{}", t("Conversation", "会话")),
    };
    // 2. 【消息导航】【右侧反馈】右侧显示紧凑位置，复制后显示字符数
    let right = if let Some(count) = state.copied {
        if crate::i18n::is_zh() {
            format!("{header_copied}已复制 {count} 个字符")
        } else {
            format!("{header_copied}Copied {count} characters")
        }
    } else {
        super::header_controls::position(state, layout)
    };
    let left = if state.message_picker.active {
        super::hover::tint_line(&super::guide_button::render(&left, true))
    } else if state.header_hover {
        super::hover::tint_line(&super::guide_button::render(&left, false))
    } else {
        left
    };
    let right_width = visible_width(&right);
    let left_budget = cols.saturating_sub(right_width + 3).max(1);
    let left = clip_to_width(&format!("{header_bg} {left}"), left_budget + 1);
    let target_width = visible_width(&left).min(cols);
    let gap = cols.saturating_sub(visible_width(&left) + right_width + 1);
    let line = format!(
        "{left}{header_bg}{}{right}{header_bg} {RESET}",
        " ".repeat(gap)
    );
    (clip_to_width(&line, cols), target_width)
}

/// 【全屏画布】【预览文本】生成悬停预览文本：消息序号与摘要。
/// 参数: `mark` 为被悬停标记，`summary` 为摘要，`state` 为全屏状态；返回: 预览卡片纯文本。
fn preview_text(mark: super::overview::RailMark, summary: &str, state: &FullscreenState) -> String {
    let total = state.document.anchors.len();
    if mark.last > mark.anchor {
        format!("{}-{}/{total}  {summary}", mark.anchor + 1, mark.last + 1)
    } else {
        format!("{}/{total}  {summary}", mark.anchor + 1)
    }
}

/// 【全屏画布】【预览叠加】把预览卡片右对齐叠在正文行上。
/// 参数: `line` 为正文行，`text` 为预览纯文本，`width` 为正文宽度，`theme` 为画布主题；返回: 定宽行。
fn overlay_preview(line: &str, text: &str, width: usize, theme: &CanvasTheme) -> String {
    let natural = unicode_width::UnicodeWidthStr::width(text) + 2;
    let box_width = natural
        .min(PREVIEW_MAX_WIDTH)
        .min(width.saturating_sub(4))
        .max(8);
    let inner = crate::render::clip_to_width(text, box_width.saturating_sub(2), "…");
    let padding =
        box_width.saturating_sub(unicode_width::UnicodeWidthStr::width(inner.as_str()) + 2);
    let card = format!(
        "{}{} {inner}{} {RESET}",
        theme.preview_bg,
        theme.preview_text,
        " ".repeat(padding)
    );
    let left = fit(line, width.saturating_sub(box_width));
    format!("{left}{card}")
}

/// 【全屏画布】【定宽裁剪】截断并补齐正文行到固定宽度。
/// 参数: `line` 为 ANSI 正文行，`width` 为目标宽度；返回: 定宽行。
fn fit(line: &str, width: usize) -> String {
    let clipped = clip_to_width(line, width);
    let padding = width.saturating_sub(visible_width(&clipped));
    format!("{clipped}{}", " ".repeat(padding))
}

/// 【全屏画布】【轨道字形】选择一行概览轨道的标记字形。
/// 参数: `mark` 为标记下标，`active` 为当前标记，`hovered` 为悬停标记，`theme` 为主题；返回: 两列宽轨道文本。
fn rail_glyph(
    mark: Option<usize>,
    active: Option<usize>,
    hovered: Option<usize>,
    theme: &CanvasTheme,
) -> String {
    match mark {
        Some(index) if hovered == Some(index) => format!("{}━━{RESET}", theme.rail_hover),
        Some(index) if active == Some(index) => format!("{}━━{RESET}", theme.rail_active),
        Some(_) => format!("{}╶─{RESET}", theme.rail_idle),
        None => "  ".to_string(),
    }
}

/// 【全屏画布】【滑块区间】计算滚动条滑块所在的行范围。
/// 参数: `state` 为全屏状态，`body_height` 为正文区行数；返回: 滑块区间，不足一屏时为空。
fn scroll_thumb(state: &FullscreenState, body_height: usize) -> Option<(usize, usize)> {
    let total = state.document.lines.len();
    if total <= body_height || body_height == 0 {
        return None;
    }
    let size = (body_height * body_height / total).max(1);
    let travel = body_height - size;
    let max = total - body_height;
    let top = state.scroll.min(max) * travel / max;
    Some((top, top + size))
}

/// 【全屏画布】【滚动条字形】选择一行主滚动条字形。
/// 参数: `row` 为相对正文顶部行，`thumb` 为滑块范围，`theme` 为主题；返回: 单列滚动条文本。
fn scrollbar_glyph(row: usize, thumb: Option<(usize, usize)>, theme: &CanvasTheme) -> String {
    match thumb {
        Some((top, bottom)) if row >= top && row < bottom => format!("{}█{RESET}", theme.thumb),
        Some(_) => format!("{}│{RESET}", theme.track),
        None => " ".to_string(),
    }
}

/// 【全屏画布】【轨道映射】把滚动条上的点击位置换算成文档首行。
/// 参数: `state` 为全屏状态，`row` 为相对正文顶部的行，`body_height` 为正文区行数；返回: 目标首行。
pub(super) fn scroll_for_track_row(
    state: &FullscreenState,
    row: usize,
    body_height: usize,
) -> usize {
    let max = state.max_scroll(body_height);
    if body_height <= 1 {
        return max;
    }
    row.min(body_height - 1) * max / (body_height - 1)
}

#[cfg(test)]
#[path = "scrollbar_visibility_tests.rs"]
mod visibility_tests;
