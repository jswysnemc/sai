use super::model::Picker;
use super::rows;
use crate::config_tui::layout::content_frame;
use crate::config_tui::theme::{
    help_line, ACCENT, BOLD, BRAND, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT, CORNER_TOP_LEFT,
    CORNER_TOP_RIGHT, DIM, LINE_HORIZONTAL, LINE_VERTICAL, MUTED, RESET,
};
use crate::config_tui::ui::{begin_synced_frame, display_width, end_synced_frame, truncate};
use crate::i18n::text as t;
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

/// 【会话选择】【绘制】绘制紧凑圆角面板：搜索、分组列表、选中详情与快捷键。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `picker`: 列表状态
/// - `size`: 终端列数与行数
///
/// 返回:
/// - 输出结果
pub(super) fn draw(stdout: &mut io::Stdout, picker: &Picker, size: (u16, u16)) -> Result<()> {
    let (cols, rows) = size;
    // 1. 面板高度按全量会话计算，切换范围或搜索时外框不跳动
    let list_rows = picker.max_rows().max(MIN_LIST_ROWS);
    let content_rows = (list_rows + CHROME_ROWS).min(usize::from(u16::MAX)) as u16;
    let frame = content_frame(cols, rows, content_rows, PICKER_MAX_WIDTH);
    let inner = usize::from(frame.width.saturating_sub(4)).max(1);
    let list_height = usize::from(frame.height.saturating_sub(2))
        .saturating_sub(CHROME_ROWS)
        .max(1);

    // 2. 组装框内各行：搜索、分隔、列表窗口、分隔、详情
    let (search, cursor_col) = rows::search_line(picker, inner);
    let (body, selected) = rows::body(picker, inner);
    let start = selected
        .saturating_sub(list_height.saturating_sub(1))
        .min(body.len().saturating_sub(list_height));
    let mut inside = vec![search, divider(inner)];
    inside.extend(body.into_iter().skip(start).take(list_height));
    inside.resize(list_height + 2, String::new());
    inside.push(divider(inner));
    inside.extend(rows::detail_lines(picker, inner));

    // 3. 套上边框：标题嵌顶边，快捷键嵌底边
    let mut lines = Vec::with_capacity(inside.len() + 2);
    lines.push(top_border(picker, frame.width));
    lines.extend(inside.iter().map(|line| boxed(line, inner)));
    lines.push(bottom_border(cols, frame.width));
    lines.truncate(usize::from(rows));
    let indent = " ".repeat(usize::from(frame.x));
    let lines = lines
        .into_iter()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>();

    // 整帧清屏重画放在同步更新内，尺寸或范围变化后不会残留旧行
    begin_synced_frame(stdout)?;
    queue!(stdout, Hide, Clear(ClearType::All))?;
    paint_changed_rows(stdout, frame.y, usize::from(cols), &lines, None)?;
    // 4. 光标停在搜索输入末尾，提示可直接输入
    let cursor_x = frame.x.saturating_add(2) + cursor_col.min(inner) as u16;
    queue!(stdout, MoveTo(cursor_x, frame.y.saturating_add(1)), Show)?;
    end_synced_frame(stdout)
}

/// 生成嵌入标题与会话数量的顶边框。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 外框宽度
///
/// 返回:
/// - 顶边框行
fn top_border(picker: &Picker, width: u16) -> String {
    let title = t("Resume session", "恢复会话");
    let count = format!("{}/{}", picker.visible.len(), picker.targets.len());
    // 固定字符：左角、短横、菱形、右角与四处空格
    let label_width = display_width(title) + display_width(&count) + 8;
    let fill = usize::from(width).saturating_sub(label_width);
    format!(
        "{DIM}{CORNER_TOP_LEFT}{LINE_HORIZONTAL}{RESET} {BRAND}◆{RESET} {ACCENT}{BOLD}{title}{RESET} {MUTED}{count}{RESET} {DIM}{}{CORNER_TOP_RIGHT}{RESET}",
        LINE_HORIZONTAL.to_string().repeat(fill)
    )
}

/// 生成嵌入快捷键说明的底边框，窄终端使用短版本。
///
/// 参数:
/// - `cols`: 终端列数
/// - `width`: 外框宽度
///
/// 返回:
/// - 底边框行
fn bottom_border(cols: u16, width: u16) -> String {
    let pairs: &[(&str, &str)] = if cols < 70 {
        &[
            ("Tab", t("scope", "范围")),
            ("Enter", t("resume", "恢复")),
            ("Esc", t("cancel", "取消")),
        ]
    } else {
        &[
            ("Tab", t("switch scope", "切换范围")),
            ("↑↓", t("select", "选择")),
            ("Enter", t("resume", "恢复")),
            ("Esc", t("cancel", "取消")),
        ]
    };
    let help = help_line(pairs);
    let plain = pairs
        .iter()
        .map(|(key, text)| format!("{key} {text}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let fill = usize::from(width).saturating_sub(display_width(&plain) + 5);
    format!(
        "{DIM}{CORNER_BOTTOM_LEFT}{LINE_HORIZONTAL}{RESET} {help} {DIM}{}{CORNER_BOTTOM_RIGHT}{RESET}",
        LINE_HORIZONTAL.to_string().repeat(fill)
    )
}

/// 生成框内弱化分隔线。
///
/// 参数:
/// - `inner`: 框内宽度
///
/// 返回:
/// - 分隔线文本
fn divider(inner: usize) -> String {
    format!("{DIM}{}{RESET}", "┄".repeat(inner))
}

/// 给框内一行加左右边框并补齐宽度。
///
/// 参数:
/// - `line`: 带样式的行内容
/// - `inner`: 框内宽度
///
/// 返回:
/// - 带边框的完整行
fn boxed(line: &str, inner: usize) -> String {
    let visible = crate::cli::repl_text::visible_width(line);
    let line = if visible > inner {
        truncate(&strip_ansi(line), inner)
    } else {
        line.to_string()
    };
    let padding = inner.saturating_sub(crate::cli::repl_text::visible_width(&line));
    format!(
        "{DIM}{LINE_VERTICAL}{RESET} {line}{RESET}{} {DIM}{LINE_VERTICAL}{RESET}",
        " ".repeat(padding)
    )
}

/// 去除 ANSI 序列，仅在行超宽需要截断时使用。
///
/// 参数:
/// - `value`: 带样式文本
///
/// 返回:
/// - 纯文本
fn strip_ansi(value: &str) -> String {
    crate::cli::repl_text::strip_terminal_control_sequences(value)
}
