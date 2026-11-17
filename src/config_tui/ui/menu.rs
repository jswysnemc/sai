//! 【配置界面】【菜单渲染】绘制近全屏主从菜单并生成列表行与底栏按键的鼠标命中区。

use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

use super::super::layout::{content_frame, master_detail_widths, scroll_start, FrameRect};
use super::super::mouse::{
    read_menu_input, HitButtons, HitRect, ListHitbox, ListMouseState, MenuInput,
};
use super::super::search::SEARCH_CURSOR_MARK;
use super::super::theme::{
    help_line, help_separator, row_marks, ThemePalette, LINE_VERTICAL, RESET,
};
use super::{
    ansi_width, begin_synced_frame, draw_box, draw_inner_divider, draw_scroll_indicator,
    draw_status_bar_with_cursor, draw_wrapped_detail, end_synced_frame, fit_status_bar, pad,
    truncate, truncate_ansi, wrap_text,
};
use crate::i18n::text as t;

/// 菜单面板最大宽度：选项与说明两栏在此宽度内已足够舒展。
const MENU_MAX_WIDTH: u16 = 100;
/// 说明栏最少保留的行数，切换选项时面板高度不随说明长短跳动。
const MENU_MIN_DETAIL_ROWS: usize = 4;

/// 菜单帧的列表与底栏快捷键鼠标命中区域。
#[derive(Clone, Debug, Default)]
pub(crate) struct MenuHitbox {
    pub list: ListHitbox,
    pub footer: HitButtons<KeyCode>,
}

impl MenuHitbox {
    /// 【配置界面】【菜单输入】读取本帧关联的键盘、点击、滚轮、粘贴与窗口变化事件。
    ///
    /// 参数:
    /// - `mouse`: 列表悬停与单击追踪状态
    /// - `selected`: 当前选中下标
    /// - `direct_activate`: 为 true 时左键单击直接激活条目
    ///
    /// 返回:
    /// - 归一化后的菜单输入动作
    pub(crate) fn read(
        &self,
        mouse: &mut ListMouseState,
        selected: usize,
        direct_activate: bool,
    ) -> Result<MenuInput> {
        read_menu_input(
            mouse,
            &self.list,
            selected,
            direct_activate,
            Some(&self.footer),
        )
    }
}

/// 【配置界面】【单栏菜单】绘制不含右侧说明栏的近全屏菜单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 左侧选项
/// - `selected`: 选中下标
/// - `status`: 底栏自定义状态；空则用默认快捷键说明
///
/// 返回:
/// - 绘制结果
#[allow(dead_code)]
pub(crate) fn draw_menu(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    selected: usize,
    status: &str,
) -> Result<()> {
    draw_menu_with_details(stdout, title, options, &[], selected, status, "")
}

/// 【配置界面】【详情菜单】绘制无鼠标悬停态的主从详情菜单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 左侧选项
/// - `details`: 与选项对齐的说明；空切片则单栏铺满
/// - `selected`: 选中下标
/// - `status`: 底栏状态
/// - `subtitle`: 标题下的摘要行
///
/// 返回:
/// - 绘制结果
#[allow(dead_code)]
pub(crate) fn draw_menu_with_details(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    details: &[String],
    selected: usize,
    status: &str,
    subtitle: &str,
) -> Result<()> {
    draw_menu_with_hover(
        stdout, title, options, details, selected, None, None, status, subtitle,
    )?;
    Ok(())
}

/// 【配置界面】【交互菜单】绘制主从菜单并应用鼠标行悬停、底栏悬停及搜索光标显示。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 选项切片
/// - `details`: 右侧说明切片
/// - `selected`: 当前选中下标
/// - `mouse`: 鼠标悬停与点击状态
/// - `status`: 底栏状态文本
/// - `subtitle`: 顶栏副标题
///
/// 返回:
/// - 本帧菜单鼠标命中区域
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_menu_interactive(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    details: &[String],
    selected: usize,
    mouse: &ListMouseState,
    status: &str,
    subtitle: &str,
) -> Result<MenuHitbox> {
    draw_menu_with_hover(
        stdout,
        title,
        options,
        details,
        selected,
        mouse.hovered,
        mouse.footer_hover,
        status,
        subtitle,
    )
}

/// 【配置界面】【完整绘制】绘制主从菜单与悬停高亮点，返回列表与底栏命中区域。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 左侧选项切片
/// - `details`: 右侧详情切片
/// - `selected`: 选中下标
/// - `hovered`: 鼠标悬停行下标
/// - `hovered_footer`: 鼠标悬停底栏按钮矩形
/// - `status`: 底栏文本
/// - `subtitle`: 副标题文本
///
/// 返回:
/// - 列表与底栏热区集合
#[allow(clippy::too_many_arguments)]
fn draw_menu_with_hover(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    details: &[String],
    selected: usize,
    hovered: Option<usize>,
    hovered_footer: Option<HitRect>,
    status: &str,
    subtitle: &str,
) -> Result<MenuHitbox> {
    let palette = ThemePalette::current();
    let (cols, rows) = terminal::size()?;
    let frame = menu_frame(cols, rows, options, details, !subtitle.is_empty());

    begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, frame.x, frame.y, frame.width, frame.height, title)?;

    let inner_x = frame.x.saturating_add(2);
    let inner_w = frame.width.saturating_sub(4);
    let mut body_y = frame.y.saturating_add(1);
    if !subtitle.is_empty() {
        let rendered_subtitle = if subtitle.contains('\x1b') {
            truncate_ansi(subtitle, inner_w as usize)
        } else {
            format!(
                "{}{}{RESET}",
                palette.muted,
                truncate(subtitle, inner_w as usize)
            )
        };
        queue!(
            stdout,
            MoveTo(inner_x, body_y),
            Print(crate::render::readable_terminal_text(&rendered_subtitle))
        )?;
        body_y = body_y.saturating_add(1);
        // 1. 【配置界面】【副标题分隔】副标题与正文之间绘制弱化横线划定边界
        draw_inner_divider(stdout, frame.x, body_y, frame.width)?;
        body_y = body_y.saturating_add(1);
    }

    // 2. 【配置界面】【主从分栏】底部预留状态条行，按内容宽度划分左侧列表与右侧详情
    let body_bottom = frame.y.saturating_add(frame.height.saturating_sub(2));
    let body_h = body_bottom.saturating_sub(body_y).max(1);
    let (left_w, right_w) = if details.is_empty() {
        (inner_w, 0)
    } else {
        master_detail_widths(inner_w)
    };

    let visible_rows = body_h.max(1) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        let row_y = body_y.saturating_add(row as u16);
        queue!(stdout, MoveTo(inner_x, row_y))?;
        if index >= options.len() {
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(" ".repeat(left_w as usize)).to_string()
                ))
            )?;
            continue;
        }
        let is_hovered = hovered == Some(index);
        let (bar, style) = row_marks(index == selected, is_hovered);
        let label = pad(&options[index], left_w.saturating_sub(2) as usize);
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!("{bar}{style} {label}{RESET}")).to_string()
            ))
        )?;
    }
    if right_w > 0 && options.len() <= visible_rows {
        // 3. 【配置界面】【中轴分隔】列表未溢出时在左右栏之间绘制柔和竖线，溢出时切换为滚动指示条
        for row in 0..body_h {
            queue!(
                stdout,
                MoveTo(inner_x.saturating_add(left_w), body_y.saturating_add(row)),
                Print(crate::render::readable_terminal_text(&format!(
                    "{}{LINE_VERTICAL}{RESET}",
                    palette.dim
                )))
            )?;
        }
    } else {
        draw_scroll_indicator(
            stdout,
            inner_x.saturating_add(left_w),
            body_y,
            body_h,
            options.len(),
            start,
            visible_rows,
        )?;
    }

    if right_w > 0 {
        let detail_x = inner_x.saturating_add(left_w).saturating_add(2);
        let detail = details.get(selected).map(String::as_str).unwrap_or("");
        draw_wrapped_detail(stdout, detail_x, body_y, right_w, body_h, detail)?;
    }

    // 4. 【配置界面】【底栏与光标同步】绘制底栏热区，并在搜索编辑态下显示真实硬件光标
    let rendered_status = menu_help(status);
    let bar_cursor = draw_status_bar_with_cursor(stdout, &frame, &rendered_status, hovered_footer)?;
    let footer = status_bar_hits(&frame, &rendered_status);
    if let Some((cx, cy)) = bar_cursor {
        queue!(stdout, Show, MoveTo(cx, cy))?;
    } else {
        queue!(stdout, Hide)?;
    }
    end_synced_frame(stdout)?;
    Ok(MenuHitbox {
        list: ListHitbox {
            frame: HitRect::from(frame),
            list: HitRect {
                x: inner_x,
                y: body_y,
                width: left_w,
                height: visible_rows as u16,
            },
            start,
            total: options.len(),
            visible_rows,
        },
        footer,
    })
}

/// 【配置界面】【菜单外框计算】按选项数量与最长说明计算菜单外框，条目少时面板随内容收缩。
///
/// 参数:
/// - `cols`: 终端列数
/// - `rows`: 终端行数
/// - `options`: 左侧选项
/// - `details`: 与选项对齐的说明
/// - `has_subtitle`: 是否有副标题行
///
/// 返回:
/// - 菜单外框矩形
fn menu_frame(
    cols: u16,
    rows: u16,
    options: &[String],
    details: &[String],
    has_subtitle: bool,
) -> FrameRect {
    let probe = content_frame(cols, rows, 1, MENU_MAX_WIDTH);
    let inner_w = probe.width.saturating_sub(4);
    let right_w = if details.is_empty() {
        0
    } else {
        master_detail_widths(inner_w).1
    };
    let detail_rows = if right_w == 0 {
        0
    } else {
        details
            .iter()
            .map(|detail| wrap_text(detail, right_w as usize).len() + 2)
            .max()
            .unwrap_or(0)
            .max(MENU_MIN_DETAIL_ROWS)
    };
    let header_rows = if has_subtitle { 2 } else { 0 };
    let body_rows = options.len().max(detail_rows).max(1);
    let content_rows = (header_rows + body_rows + 1).min(usize::from(u16::MAX)) as u16;
    content_frame(cols, rows, content_rows, MENU_MAX_WIDTH)
}

/// 【配置界面】【默认底栏】组装菜单底部帮助条；传入非空自定义文本时原样保留。
///
/// 参数:
/// - `status`: 自定义状态栏文本
///
/// 返回:
/// - 最终用于绘制的状态栏文本
fn menu_help(status: &str) -> String {
    if status.is_empty() {
        help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("open", "打开")),
            ("q", t("back", "返回")),
        ])
    } else {
        status.to_string()
    }
}

/// 【配置界面】【底栏按钮】按实际裁剪后的底栏分段生成鼠标可点按键命中表。
///
/// 参数:
/// - `frame`: 外框矩形
/// - `content`: 未裁剪的状态栏原始文本
///
/// 返回:
/// - 各分段对应的键值命中区集合
pub(crate) fn status_bar_hits(frame: &FrameRect, content: &str) -> HitButtons<KeyCode> {
    let mut buttons = HitButtons::default();
    let max_width = frame.width.saturating_sub(6) as usize;
    let fitted = fit_status_bar(content, max_width);
    let separator = help_separator();
    let sep_width = ansi_width(&separator);
    let bar_y = frame.y.saturating_add(frame.height.saturating_sub(1));
    let mut col = usize::from(frame.x.saturating_add(3));

    for segment in fitted.split(&separator) {
        let seg_width = ansi_width(segment);
        let plain = strip_ansi_plain(segment);
        if let Some(token) = plain.split_whitespace().next() {
            if let Some(code) = parse_status_key_token(token) {
                if seg_width > 0 {
                    buttons.push(
                        HitRect {
                            x: col as u16,
                            y: bar_y,
                            width: seg_width as u16,
                            height: 1,
                        },
                        code,
                    );
                }
            }
        }
        col = col.saturating_add(seg_width).saturating_add(sep_width);
    }
    buttons
}

/// 【配置界面】【纯文本提取】去除文本中的 ANSI 转义序列与光标控制符以便提取纯键盘标识。
///
/// 参数:
/// - `value`: 含 ANSI 序列的文本
///
/// 返回:
/// - 纯文本字符串
pub(crate) fn strip_ansi_plain(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut index = 0usize;
    while index < value.len() {
        let ch = value[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(value, index);
            continue;
        }
        if ch != SEARCH_CURSOR_MARK {
            out.push(ch);
        }
        index += ch.len_utf8();
    }
    out
}

/// 【配置界面】【键位映射】将帮助条分段首词映射到对应的按键事件码。
///
/// 参数:
/// - `token`: 帮助分段首个词元
///
/// 返回:
/// - 可触发动作的按键码
fn parse_status_key_token(token: &str) -> Option<KeyCode> {
    match token {
        "Enter" | "y/s/Enter" | "Enter/Esc" => Some(KeyCode::Enter),
        "Space" | "Space/u" => Some(KeyCode::Char(' ')),
        "Tab" => Some(KeyCode::Tab),
        "Esc" | "n/Esc" | "Esc/n" | "q/Esc" | "Esc/q" => Some(KeyCode::Esc),
        "Backspace" | "Del" => Some(KeyCode::Backspace),
        "y" | "y/s" | "y/Y" => Some(KeyCode::Char('y')),
        "d/n" => Some(KeyCode::Char('d')),
        "s/w" | "Ctrl+s" => Some(KeyCode::Char('s')),
        _ => {
            let mut chars = token.chars();
            let ch = chars.next()?;
            if chars.next().is_none() && (ch.is_ascii_alphanumeric() || "/?+-".contains(ch)) {
                Some(KeyCode::Char(ch))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bar_hits_tracks_visible_action_columns() {
        let frame = FrameRect {
            x: 1,
            y: 0,
            width: 90,
            height: 20,
        };
        let status = help_line(&[
            ("↑↓", "move"),
            ("Space", "cycle"),
            ("Enter", "open"),
            ("s", "save"),
            ("q", "quit"),
        ]);
        let hits = status_bar_hits(&frame, &status);
        assert_eq!(hits.hit(5, 19), None);
        assert_eq!(hits.hit(14, 19), Some(KeyCode::Char(' ')));
        assert_eq!(hits.hit(28, 19), Some(KeyCode::Enter));
        assert_eq!(hits.hit(41, 19), Some(KeyCode::Char('s')));
        assert_eq!(hits.hit(50, 19), Some(KeyCode::Char('q')));
    }
}
