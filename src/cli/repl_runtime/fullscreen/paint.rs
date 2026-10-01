//! 全屏会话视图的逐行组装：浮动标题、正文、概览轨道、悬停预览与滚动条。
//!
//! 纯函数：只根据状态与分区生成每一行的 ANSI 文本，由调用方统一做差异绘制。

use super::layout::FullscreenLayout;
use super::overview::{mark_for_anchor, rail_marks};
use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::i18n::text as t;

/// 浮动标题底色，与输入框同一档深灰。
const HEADER_BG: &str = "\x1b[48;5;235m";
/// 标题中的消息序号。
const HEADER_INDEX: &str = "\x1b[38;2;94;196;168m";
/// 标题正文。
const HEADER_TEXT: &str = "\x1b[38;5;252m";
/// 标题右侧提示。
const HEADER_HINT: &str = "\x1b[38;5;244m";
/// 新输出提示。
const HEADER_UNSEEN: &str = "\x1b[1m\x1b[38;2;94;196;168m";
/// 轨道普通标记。
const RAIL_IDLE: &str = "\x1b[38;5;240m";
/// 轨道当前标记。
const RAIL_ACTIVE: &str = "\x1b[38;2;94;196;168m";
/// 轨道悬停标记。
const RAIL_HOVER: &str = "\x1b[38;5;252m";
/// 滚动条轨道与滑块。
const TRACK: &str = "\x1b[38;5;237m";
const THUMB: &str = "\x1b[38;5;245m";
/// 悬停预览卡片。
const PREVIEW_BG: &str = "\x1b[48;5;237m";
const PREVIEW_TEXT: &str = "\x1b[38;5;252m";
const RESET: &str = "\x1b[0m";
/// 悬停预览最大宽度。
const PREVIEW_MAX_WIDTH: usize = 48;

/// 组装结果：屏幕行与标题上可点击的“新输出”区域。
pub(super) struct PaintedRows {
    /// 从第 0 行到正文末行的屏幕行（不含输入框）
    pub(super) rows: Vec<String>,
    /// “新输出”提示所在列范围 [start, end)
    pub(super) unseen_cols: Option<(u16, u16)>,
    /// 正文末行“回到底部”按钮所在列范围 [start, end)
    pub(super) bottom_button: Option<(u16, u16)>,
}

/// 组装标题与正文区的全部屏幕行。
///
/// 参数:
/// - `state`: 全屏状态（正文已换入）
/// - `layout`: 屏幕分区
///
/// 返回:
/// - 逐行 ANSI 文本与标题点击区域
pub(super) fn compose(state: &FullscreenState, layout: &FullscreenLayout) -> PaintedRows {
    let (header, unseen_cols) = header_line(state, layout);
    let mut rows = vec![header];
    let body_height = usize::from(layout.body_height);
    let content_width = usize::from(layout.content_width);
    // 1. 右侧辅助列：概览标记、滚动条
    let marks = rail_marks(state.document.anchors.len(), body_height);
    let active = state
        .current_anchor()
        .and_then(|anchor| mark_for_anchor(&marks, anchor));
    // 没有概览轨道时（窄终端）不显示悬停预览
    let hovered = state
        .hover
        .filter(|_| layout.rail_col.is_some())
        .and_then(|anchor| mark_for_anchor(&marks, anchor));
    let thumb = scroll_thumb(state, body_height);
    // 2. 悬停预览卡片停在被悬停标记的同一行
    let preview = hovered.and_then(|index| {
        let anchor = state.document.anchors.get(marks[index].anchor)?;
        Some((
            marks[index].row,
            preview_text(marks[index], anchor.summary.as_str(), state),
        ))
    });
    // 3. 跨行公式图片按窗口边界裁剪，滚动时不会整张消失
    let window = super::image_window::window_lines(
        &state.document.lines,
        state.scroll,
        body_height,
        crate::render::terminal_image::kitty_cell_pixel_height(),
    );
    let mut bottom_button = None;
    for (row, line) in window.iter().enumerate() {
        let line = line.as_str();
        let body = match &preview {
            Some((preview_row, text)) if *preview_row == row => {
                overlay_preview(line, text, content_width)
            }
            _ => fit(line, content_width),
        };
        // 4. 拖动选区以反色标出
        let body = match state
            .selection
            .and_then(|selection| selection.cols_on_row(state.scroll + row, content_width))
        {
            Some((from, to)) => super::selection::highlight_cols(&body, from, to),
            None => body,
        };
        // 5. 离开底部时在正文末行右侧叠加“回到底部”按钮；悬停预览所在行不叠加
        let is_last = row + 1 == body_height;
        let hovered_here = matches!(&preview, Some((preview_row, _)) if *preview_row == row);
        let body = if is_last && !hovered_here {
            let (line, cols) = super::bottom_button::overlay(&body, state, content_width);
            bottom_button = cols;
            line
        } else {
            body
        };
        let mut output = body;
        if layout.rail_col.is_some() {
            let mark = marks.iter().position(|mark| mark.row == row);
            output.push(' ');
            output.push_str(&rail_glyph(mark, active, hovered));
        }
        output.push_str(&scrollbar_glyph(row, thumb));
        rows.push(output);
    }
    PaintedRows {
        rows,
        unseen_cols,
        bottom_button,
    }
}

/// 生成浮动标题：当前用户消息序号与摘要，右侧为新输出提示或快捷键。
///
/// 参数:
/// - `state`: 全屏状态
/// - `layout`: 屏幕分区
///
/// 返回:
/// - 标题行与“新输出”区域
fn header_line(state: &FullscreenState, layout: &FullscreenLayout) -> (String, Option<(u16, u16)>) {
    let cols = usize::from(layout.cols);
    let total = state.document.anchors.len();
    // 1. 左侧：当前所在用户消息
    let left = match state.current_anchor() {
        Some(index) => format!(
            "{HEADER_INDEX}● {}/{total}{RESET}{HEADER_BG} {HEADER_TEXT}{}",
            index + 1,
            state.document.anchors[index].summary
        ),
        None if total > 0 => format!(
            "{HEADER_INDEX}● 0/{total}{RESET}{HEADER_BG} {HEADER_TEXT}{}",
            t("Start of conversation", "会话开头")
        ),
        None => format!("{HEADER_TEXT}{}", t("Conversation", "会话")),
    };
    // 2. 右侧：离开底部且有新输出时提示，否则给出快捷键
    let (right, clickable) = if let Some(count) = state.copied {
        (
            if crate::i18n::is_zh() {
                format!("{HEADER_UNSEEN}已复制 {count} 个字符")
            } else {
                format!("{HEADER_UNSEEN}Copied {count} characters")
            },
            false,
        )
    } else if state.unseen {
        (
            format!("{HEADER_UNSEEN}{}", t("↓ New output", "↓ 有新输出")),
            true,
        )
    } else if cols >= 100 {
        (
            format!(
                "{HEADER_HINT}{}",
                t(
                    "Click to fold · Drag to copy · Alt+↑↓ messages · Ctrl+O exit",
                    "点击展开/收起 · 拖动复制 · Alt+↑↓ 切换消息 · Ctrl+O 退出"
                )
            ),
            false,
        )
    } else {
        (
            format!("{HEADER_HINT}{}", t("Ctrl+O exit", "Ctrl+O 退出")),
            false,
        )
    };
    let right_width = visible_width(&right);
    let left_budget = cols.saturating_sub(right_width + 3).max(1);
    let left = clip_to_width(&format!("{HEADER_BG} {left}"), left_budget + 1);
    let gap = cols.saturating_sub(visible_width(&left) + right_width + 1);
    let line = format!(
        "{left}{HEADER_BG}{}{right}{HEADER_BG} {RESET}",
        " ".repeat(gap)
    );
    let start = (cols - right_width - 1) as u16;
    let unseen_cols = clickable.then_some((start, start + right_width as u16));
    (clip_to_width(&line, cols), unseen_cols)
}

/// 生成悬停预览文本：消息序号与摘要。
///
/// 参数:
/// - `mark`: 被悬停的标记
/// - `summary`: 首条消息摘要
/// - `state`: 全屏状态
///
/// 返回:
/// - 预览卡片纯文本
fn preview_text(mark: super::overview::RailMark, summary: &str, state: &FullscreenState) -> String {
    let total = state.document.anchors.len();
    if mark.last > mark.anchor {
        format!("{}-{}/{total}  {summary}", mark.anchor + 1, mark.last + 1)
    } else {
        format!("{}/{total}  {summary}", mark.anchor + 1)
    }
}

/// 把预览卡片右对齐叠在正文行上。
///
/// 参数:
/// - `line`: 正文行
/// - `text`: 预览纯文本
/// - `width`: 正文宽度
///
/// 返回:
/// - 叠加后的定宽行
fn overlay_preview(line: &str, text: &str, width: usize) -> String {
    // 卡片宽度贴合文字，封顶后截断，右侧紧靠概览轨道
    let natural = unicode_width::UnicodeWidthStr::width(text) + 2;
    let box_width = natural
        .min(PREVIEW_MAX_WIDTH)
        .min(width.saturating_sub(4))
        .max(8);
    let inner = crate::render::clip_to_width(text, box_width.saturating_sub(2), "…");
    let padding =
        box_width.saturating_sub(unicode_width::UnicodeWidthStr::width(inner.as_str()) + 2);
    let card = format!(
        "{PREVIEW_BG}{PREVIEW_TEXT} {inner}{} {RESET}",
        " ".repeat(padding)
    );
    let left = fit(line, width.saturating_sub(box_width));
    format!("{left}{card}")
}

/// 截断并补齐正文行到固定宽度。
///
/// 参数:
/// - `line`: ANSI 正文行
/// - `width`: 目标宽度
///
/// 返回:
/// - 定宽行，末尾复位样式
fn fit(line: &str, width: usize) -> String {
    let clipped = clip_to_width(line, width);
    let padding = width.saturating_sub(visible_width(&clipped));
    format!("{clipped}{}", " ".repeat(padding))
}

/// 选择一行轨道的标记字形。
///
/// 参数:
/// - `mark`: 该行的标记下标
/// - `active`: 当前消息对应的标记
/// - `hovered`: 悬停的标记
///
/// 返回:
/// - 两列宽的轨道文本
fn rail_glyph(mark: Option<usize>, active: Option<usize>, hovered: Option<usize>) -> String {
    match mark {
        Some(index) if hovered == Some(index) => format!("{RAIL_HOVER}━━{RESET}"),
        Some(index) if active == Some(index) => format!("{RAIL_ACTIVE}━━{RESET}"),
        Some(_) => format!("{RAIL_IDLE}╶─{RESET}"),
        None => "  ".to_string(),
    }
}

/// 计算滚动条滑块所在的行范围。
///
/// 参数:
/// - `state`: 全屏状态
/// - `body_height`: 正文区行数
///
/// 返回:
/// - 滑块 [top, bottom)；内容不足一屏时为 None
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

/// 选择一行滚动条字形。
///
/// 参数:
/// - `row`: 相对正文顶部的行
/// - `thumb`: 滑块范围
///
/// 返回:
/// - 单列滚动条文本
fn scrollbar_glyph(row: usize, thumb: Option<(usize, usize)>) -> String {
    match thumb {
        Some((top, bottom)) if row >= top && row < bottom => format!("{THUMB}┃{RESET}"),
        Some(_) => format!("{TRACK}│{RESET}"),
        None => " ".to_string(),
    }
}

/// 把滚动条上的点击位置换算成文档首行。
///
/// 参数:
/// - `state`: 全屏状态
/// - `row`: 相对正文顶部的行
/// - `body_height`: 正文区行数
///
/// 返回:
/// - 目标首行
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
