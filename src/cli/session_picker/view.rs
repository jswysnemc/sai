use super::model::Picker;
use super::mouse::DrawHits;
use super::rows;
use crate::config_tui::layout::content_frame;
use crate::config_tui::theme::{
    BOLD, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT, CORNER_TOP_LEFT, CORNER_TOP_RIGHT,
    LINE_HORIZONTAL, LINE_VERTICAL, RESET,
};
use crate::config_tui::ui::{begin_synced_frame, display_width, end_synced_frame};
use crate::i18n::text as t;
use crate::render::panel_palette::{fg, PanelPalette};
use crate::render::readable_terminal_text;
use crate::render::terminal_rows::paint_changed_rows;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io;

/// 选择器面板最大宽度。
const PICKER_MAX_WIDTH: u16 = 100;
/// 列表区最少行数，会话很少时面板也保留基本高度。
const MIN_LIST_ROWS: usize = 4;
/// 框内固定行：搜索、分隔、分隔、详情两行。
const CHROME_ROWS: usize = 5;

/// 【会话选择】【绘制】按明暗自适应配色绘制圆角面板，更新视口偏移并返回鼠标命中几何。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `picker`: 列表状态（同步更新当前视口起始行）
/// - `size`: 终端列数与行数
///
/// 返回:
/// - 当前帧的点击与列表框滚轮命中几何
pub(super) fn draw(
    stdout: &mut io::Stdout,
    picker: &mut Picker,
    size: (u16, u16),
) -> Result<DrawHits> {
    let (cols, rows) = size;
    let palette = PanelPalette::current();
    // 1. 【会话选择】【外框尺寸】面板高度按已加载会话计算，切换范围或搜索时外框不跳动
    let list_rows = picker.max_rows().max(MIN_LIST_ROWS);
    let content_rows = (list_rows + CHROME_ROWS).min(usize::from(u16::MAX)) as u16;
    let frame = content_frame(cols, rows, content_rows, PICKER_MAX_WIDTH);
    let inner = usize::from(frame.width.saturating_sub(4)).max(1);
    let list_height = usize::from(frame.height.saturating_sub(2))
        .saturating_sub(CHROME_ROWS)
        .max(1);

    // 2. 【会话选择】【视口滚动】上方保留上下文行以露出所属工作区标题，避免焦点钉死底边
    let (search, cursor_col) = rows::search_line_with_palette(picker, inner, palette);
    let (body, selected, targets) = rows::body_with_palette(picker, inner, palette);
    let start = list_viewport_start(picker.scroll, selected, body.len(), list_height);
    picker.scroll = start;

    let list_top = frame.y.saturating_add(3);
    let list_bottom = list_top.saturating_add(list_height as u16).min(rows);
    let mut hits = targets
        .into_iter()
        .filter(|(row, _)| (start..start + list_height).contains(row))
        .map(|(row, index)| {
            (
                list_top + (row - start) as u16,
                usize::from(frame.x + 2)..usize::from(frame.x + frame.width.saturating_sub(2)),
                super::mouse::Target::Item(index),
            )
        })
        .collect::<Vec<_>>();
    let mut inside = vec![search, divider(inner, palette)];
    inside.extend(body.into_iter().skip(start).take(list_height));
    inside.resize(list_height + 2, String::new());
    inside.push(divider(inner, palette));
    inside.extend(rows::detail_lines_with_palette(picker, inner, palette));

    // 3. 【会话选择】【边框与底栏】标题嵌顶边，操作按钮及 Enter/Tab/Esc 提示嵌底边
    let mut lines = Vec::with_capacity(inside.len() + 2);
    lines.push(top_border(picker, frame.width, palette));
    lines.extend(inside.iter().map(|line| boxed(line, inner, palette)));
    let buttons = super::mouse::buttons(usize::from(frame.width.saturating_sub(4)));
    let button_row = frame.y.saturating_add(lines.len() as u16);
    for column in 0..frame.width.saturating_sub(4) {
        if let Some(target) = buttons.hit(column) {
            let x = usize::from(frame.x + 2 + column);
            hits.push((button_row, x..x + 1, target));
        }
    }
    let fill = usize::from(frame.width).saturating_sub(display_width(&buttons.text) + 4);
    let border_fg = fg(palette.border);
    let muted_fg = fg(palette.muted);
    lines.push(format!(
        "{border_fg}{CORNER_BOTTOM_LEFT} {RESET}{muted_fg}{}{RESET} {border_fg}{}{CORNER_BOTTOM_RIGHT}{RESET}",
        buttons.text,
        LINE_HORIZONTAL.to_string().repeat(fill)
    ));
    lines.truncate(usize::from(rows));
    let indent = " ".repeat(usize::from(frame.x));
    let lines = lines
        .into_iter()
        .map(|line| readable_terminal_text(&format!("{indent}{line}")))
        .collect::<Vec<_>>();

    begin_synced_frame(stdout)?;
    queue!(stdout, Hide, Clear(ClearType::All))?;
    paint_changed_rows(stdout, frame.y, usize::from(cols), &lines, None)?;
    // 4. 【会话选择】【输入法光标】将光标精准停在搜索输入末尾以对齐输入法候选窗
    let cursor_x = frame.x.saturating_add(2) + cursor_col.min(inner) as u16;
    queue!(
        stdout,
        MoveTo(cursor_x.min(cols.saturating_sub(1)), frame.y.saturating_add(1)),
        Show
    )?;
    end_synced_frame(stdout)?;
    hits.retain(|(row, _, _)| *row < rows);
    Ok(DrawHits {
        hits,
        list_rows: list_top.min(rows)..list_bottom,
        list_cols: frame.x..frame.x.saturating_add(frame.width).min(cols),
    })
}

/// 【会话选择】【视口起点】计算保留上下方上下文的列表滚动起点，向上移动时提前露出工作区标题。
///
/// 参数:
/// - `current_scroll`: 上一帧滚动起点
/// - `selected_row`: 当前选中会话在展开列表中的视觉行下标
/// - `total_rows`: 展开列表总视觉行数
/// - `list_height`: 列表框可视高度
///
/// 返回:
/// - 更新后的滚动起始行号
pub(super) fn list_viewport_start(
    current_scroll: usize,
    selected_row: usize,
    total_rows: usize,
    list_height: usize,
) -> usize {
    if list_height == 0 || total_rows <= list_height {
        return 0;
    }
    let max_scroll = total_rows - list_height;
    let top_margin = 2.min(list_height.saturating_sub(1) / 2);
    let bottom_margin = 1.min(list_height.saturating_sub(1) / 2);
    let mut start = current_scroll.min(max_scroll);
    if selected_row < start.saturating_add(top_margin) {
        start = selected_row.saturating_sub(top_margin);
    } else {
        let max_focus_row = start
            .saturating_add(list_height.saturating_sub(1))
            .saturating_sub(bottom_margin);
        if selected_row > max_focus_row {
            start = selected_row
                .saturating_add(1 + bottom_margin)
                .saturating_sub(list_height);
        }
    }
    start.min(max_scroll)
}

/// 【会话选择】【顶边框】生成嵌入标题与会话数量的圆角顶边框。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 外框宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 顶边框行
fn top_border(picker: &Picker, width: u16, palette: PanelPalette) -> String {
    let title = t("Resume session", "恢复会话");
    let mut count = format!("{}/{}", picker.visible.len(), picker.targets.len());
    if picker.all && picker.loading {
        count.push_str(t(" Loading", " 正在加载"));
    } else if picker.all && picker.load_error.is_some() {
        count.push_str(t(" Load failed", " 加载失败"));
    }
    let border_fg = fg(palette.border);
    let accent = fg(palette.accent);
    let muted = fg(palette.muted);
    let inner_budget = usize::from(width.saturating_sub(4));
    let plain_header = format!("◆ {title} {count}");
    if display_width(&plain_header) + 2 > inner_budget {
        let clipped = crate::render::clip_to_width(&plain_header, inner_budget, "…");
        let pad = inner_budget.saturating_sub(display_width(&clipped));
        return format!(
            "{border_fg}{CORNER_TOP_LEFT} {RESET}{accent}{BOLD}{clipped}{RESET} {border_fg}{}{CORNER_TOP_RIGHT}{RESET}",
            LINE_HORIZONTAL.to_string().repeat(pad)
        );
    }
    let label_width = display_width(title) + display_width(&count) + 8;
    let fill = usize::from(width).saturating_sub(label_width);
    format!(
        "{border_fg}{CORNER_TOP_LEFT}{LINE_HORIZONTAL}{RESET} {accent}◆{RESET} {accent}{BOLD}{title}{RESET} {muted}{count}{RESET} {border_fg}{}{CORNER_TOP_RIGHT}{RESET}",
        LINE_HORIZONTAL.to_string().repeat(fill)
    )
}

/// 【会话选择】【分隔线】生成框内弱化分隔线。
///
/// 参数:
/// - `inner`: 框内宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 分隔线文本
fn divider(inner: usize, palette: PanelPalette) -> String {
    format!("{}{}{RESET}", fg(palette.border), "┄".repeat(inner))
}

/// 【会话选择】【加框】给框内一行加左右竖线边框并补齐宽度。
///
/// 参数:
/// - `line`: 带样式的行内容
/// - `inner`: 框内宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 带边框的完整行
fn boxed(line: &str, inner: usize, palette: PanelPalette) -> String {
    let visible = crate::cli::repl_text::visible_width(line);
    let line = if visible > inner {
        crate::render::ansi_clip::clip_ansi_to_width(line, inner, "…")
    } else {
        line.to_string()
    };
    let padding = inner.saturating_sub(crate::cli::repl_text::visible_width(&line));
    let border_fg = fg(palette.border);
    format!(
        "{border_fg}{LINE_VERTICAL}{RESET} {line}{RESET}{} {border_fg}{LINE_VERTICAL}{RESET}",
        " ".repeat(padding)
    )
}
