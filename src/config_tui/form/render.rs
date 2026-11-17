//! 表单布局与交互帧绘制：支持连续悬停底色、长表单右侧滚动条及 80 列窄屏底部说明摘要。

use super::super::mouse::{HitButtons, HitRect, ListHitbox};
use super::super::theme::{row_marks, tint_bg, ThemePalette, BOLD, RESET};
use super::super::ui::{
    begin_synced_frame, display_width, draw_box, draw_scroll_indicator,
    draw_status_bar_interactive, end_synced_frame, pad, status_bar_hits, truncate,
};
use super::detail::{
    compact_focus_hint, draw_form_detail, field_detail_text, field_display_value,
    rendered_text_value, strip_field_ansi,
};
use super::{take_chars, Field};
use crate::config_tui::layout::{
    content_frame, form_column_widths, form_label_width, scroll_start, MAX_FRAME_WIDTH,
};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

/// 表单界面交互命中区：包含列表字段、`Save`/`Cancel` 按钮及底栏操作提示。
#[derive(Clone, Debug, Default)]
pub(super) struct FormHitbox {
    pub frame: HitRect,
    pub list: ListHitbox,
    pub save: HitRect,
    pub cancel: HitRect,
    pub footer: HitButtons<crossterm::event::KeyCode>,
}

/// 表单字段区最少保留的行数，右侧字段说明需要足够的折行空间。
const MIN_FORM_BODY_ROWS: usize = 6;

/// 【配置表单】【行文本组装】按选中、编辑与悬停态构造单行字段文本，保证悬停底色连续无断层。
///
/// 参数:
/// - `field`: 当前字段
/// - `label_col`: 标签列对齐宽度
/// - `row_w`: 当前行总列宽
/// - `is_selected`: 是否为非编辑焦点行
/// - `is_editing_row`: 是否正处于行内文本编辑态
/// - `is_hovered`: 鼠标是否正悬停在该行
/// - `revealed_secret`: 是否明文显示密钥
///
/// 返回:
/// - 可直接写入终端的整行 ANSI 文本
fn format_field_row(
    field: &Field,
    label_col: usize,
    row_w: usize,
    is_selected: bool,
    is_editing_row: bool,
    is_hovered: bool,
    revealed_secret: bool,
) -> String {
    let palette = ThemePalette::current();
    let (bar, row_style) = row_marks(is_selected, is_hovered);
    let label_text = pad(&field.label, label_col);
    let value_budget = row_w.saturating_sub(2 + label_col + 2).max(8);
    let value = field_display_value(field, revealed_secret, value_budget);
    if is_selected {
        // 1. 【配置表单】【焦点选中】选中行使用左侧异色竖条与整行统一选中底色
        let line = truncate(
            &format!("{label_text}  {}", strip_field_ansi(&value)),
            row_w.saturating_sub(2),
        );
        return format!(
            "{bar}{row_style} {}{RESET}",
            pad(&line, row_w.saturating_sub(2))
        );
    }
    let editing_marker = if is_editing_row {
        format!("{}▸{RESET}", palette.accent)
    } else {
        bar
    };
    let used = 2 + label_col + 2 + display_width(&strip_field_ansi(&value));
    let remaining = row_w.saturating_sub(used);
    let trailing = " ".repeat(remaining);
    let raw = format!(
        "{editing_marker} {}{label_text}{RESET}  {value}{trailing}",
        palette.muted
    );
    if is_hovered {
        // 2. 【配置表单】【连续悬停】在非选中悬停行的每个 RESET 后补回柔和悬停底色，消除字段与值之间的背景断层
        tint_bg(&raw, palette.hover_bg)
    } else {
        raw
    }
}

/// 【配置表单】【交互绘制】绘制表单并返回字段列表、保存/取消按钮和底栏命中几何。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 表单标题
/// - `fields`: 表单字段定义
/// - `selected`: 当前焦点下标
/// - `hovered`: 鼠标悬停条目下标（含字段与底部按钮）
/// - `footer_hover`: 底栏悬停分段矩形
/// - `editing`: 是否处于行内文本编辑态
/// - `cursors`: 各字段字符光标位置
/// - `revealed_secrets`: 密钥明文展示开关
///
/// 返回:
/// - 本帧表单的鼠标命中区域集合
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_form_interactive(
    stdout: &mut io::Stdout,
    title: &str,
    fields: &[Field],
    selected: usize,
    hovered: Option<usize>,
    footer_hover: Option<HitRect>,
    editing: bool,
    cursors: &[usize],
    revealed_secrets: &[bool],
) -> Result<FormHitbox> {
    let palette = ThemePalette::current();
    let (cols, rows) = terminal::size()?;
    let probe = content_frame(cols, rows, 1, MAX_FRAME_WIDTH);
    let (_, probe_right_w) = form_column_widths(probe.width.saturating_sub(4));
    let extra_narrow_hint = usize::from(probe_right_w == 0);
    // 1. 【配置表单】【面板高度】窄屏隐藏右侧栏时额外预留一行在底部展示聚焦字段说明摘要
    let content_rows = fields
        .len()
        .saturating_add(3 + extra_narrow_hint)
        .max(MIN_FORM_BODY_ROWS + 3 + extra_narrow_hint);
    let frame = content_frame(
        cols,
        rows,
        content_rows.min(usize::from(u16::MAX)) as u16,
        MAX_FRAME_WIDTH,
    );
    let x = frame.x;
    let y = frame.y;
    let width = frame.width;
    let height = frame.height;

    begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, title)?;
    let inner_x = x.saturating_add(2);
    let inner_w = width.saturating_sub(4);
    let (left_w, right_w) = form_column_widths(inner_w);

    let show_bottom_hint = right_w == 0 && height >= 9;
    let list_top = y.saturating_add(2);
    let button_y = y.saturating_add(height.saturating_sub(2));
    let hint_y = button_y.saturating_sub(1);
    let list_bottom = if show_bottom_hint {
        hint_y
    } else {
        button_y.saturating_sub(1)
    };
    let body_h = list_bottom.saturating_sub(list_top).max(1);
    let visible_rows = body_h as usize;

    // 2. 【配置表单】【长表单滚动条】单栏窄屏且字段溢出时让出右缘 1 列放置滚动条，双栏时置于栏间空隙
    let has_scroll = fields.len() > visible_rows;
    let row_w = if right_w == 0 && has_scroll {
        left_w.saturating_sub(1) as usize
    } else {
        left_w as usize
    };
    let longest_label = fields
        .iter()
        .filter(|field| !field.section)
        .map(|field| display_width(&field.label))
        .max()
        .unwrap_or(8);
    let label_col = form_label_width(longest_label, row_w);
    let start = scroll_start(selected.min(fields.len().saturating_sub(1)), visible_rows);
    let mut cursor = None;
    for row in 0..visible_rows {
        let index = start + row;
        let row_y = list_top.saturating_add(row as u16);
        queue!(stdout, MoveTo(inner_x, row_y))?;
        if index >= fields.len() {
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(" ".repeat(row_w)).to_string()
                ))
            )?;
            continue;
        }
        let field = &fields[index];
        if field.section {
            let name = truncate(&field.label, row_w.saturating_sub(6));
            let used = 4 + display_width(&name) + 1;
            let tail = row_w.saturating_sub(used);
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(format!(
                        "{}── {RESET}{}{BOLD}{name}{RESET} {}{}{RESET}",
                        palette.dim,
                        palette.brand,
                        palette.dim,
                        "─".repeat(tail)
                    ))
                    .to_string()
                ))
            )?;
            continue;
        }
        let is_selected = index == selected && !editing;
        let is_editing_row = index == selected && editing;
        let is_hovered = hovered == Some(index);
        let rendered_row = format_field_row(
            field,
            label_col,
            row_w,
            is_selected,
            is_editing_row,
            is_hovered,
            revealed_secrets[index],
        );
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(&rendered_row))
        )?;
        if is_editing_row {
            let value_budget = row_w.saturating_sub(2 + label_col + 2).max(8);
            let rendered_value = rendered_text_value(field, revealed_secrets[index]);
            let cursor_text = take_chars(&rendered_value, cursors[index]);
            let cursor_x = inner_x
                + 2
                + label_col as u16
                + 2
                + display_width(&truncate(&cursor_text, value_budget)) as u16;
            cursor = Some((
                cursor_x.min(inner_x.saturating_add((row_w as u16).saturating_sub(1))),
                row_y,
            ));
        }
    }
    let scroll_x = if right_w == 0 {
        inner_x.saturating_add(row_w as u16)
    } else {
        inner_x.saturating_add(left_w)
    };
    draw_scroll_indicator(
        stdout,
        scroll_x,
        list_top,
        body_h,
        fields.len(),
        start,
        visible_rows,
    )?;

    // 3. 【配置表单】【宽窄屏说明切换】宽屏绘制右侧详情栏，窄屏在按钮上方单行展示当前聚焦说明
    if right_w > 0 {
        let detail_x = inner_x.saturating_add(left_w).saturating_add(2);
        let detail = if selected < fields.len() {
            field_detail_text(&fields[selected])
        } else if selected == fields.len() {
            t(
                "Save writes these values into the in-memory config. Use Save & Exit on the main menu to persist to disk.",
                "保存会把这些值写入内存中的配置。主菜单的「保存并退出」才会落盘。",
            )
            .to_string()
        } else {
            t(
                "Cancel discards edits in this form and returns to the previous screen.",
                "取消会丢弃本表单的修改并返回上一屏。",
            )
            .to_string()
        };
        draw_form_detail(stdout, detail_x, list_top, right_w, body_h, &detail)?;
    } else if show_bottom_hint {
        let hint = compact_focus_hint(fields, selected);
        let hint_line = truncate(&format!("◈ {hint}"), inner_w as usize);
        queue!(
            stdout,
            MoveTo(inner_x, hint_y),
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{}{}{RESET}",
                    palette.dim,
                    pad(&hint_line, inner_w as usize)
                ))
                .to_string()
            ))
        )?;
    }

    let save_hovered = hovered == Some(fields.len());
    let cancel_hovered = hovered == Some(fields.len() + 1);
    let save_width = draw_form_button(
        stdout,
        inner_x,
        button_y,
        t("Save", "保存"),
        selected == fields.len() && !editing,
        save_hovered,
        true,
    )?;
    let cancel_x = inner_x.saturating_add(save_width).saturating_add(2);
    let cancel_width = draw_form_button(
        stdout,
        cancel_x,
        button_y,
        t("Cancel", "取消"),
        selected == fields.len() + 1 && !editing,
        cancel_hovered,
        false,
    )?;

    let help = if selected < fields.len() && editing && fields[selected].secret {
        super::super::theme::help_line(&[
            ("Enter/Esc", t("done", "结束")),
            ("Tab", t("next", "下一项")),
            (
                "Ctrl+R",
                if revealed_secrets[selected] {
                    t("hide secret", "隐藏明文")
                } else {
                    t("reveal secret", "显示明文")
                },
            ),
        ])
    } else if selected < fields.len() && editing {
        super::super::theme::help_line(&[
            ("Enter/Esc", t("done", "结束编辑")),
            ("Tab", t("next", "下一项")),
            ("←→", t("cursor", "光标")),
            ("Home/End", t("jump", "行首尾")),
        ])
    } else {
        super::super::theme::help_line(&[
            ("↑↓/Tab", t("move", "切换")),
            ("Enter", t("edit", "编辑")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    };
    draw_status_bar_interactive(stdout, &frame, &help, footer_hover)?;
    let footer = status_bar_hits(&frame, &help);
    if let Some((cx, cy)) = cursor {
        queue!(stdout, Show, MoveTo(cx, cy))?;
    } else {
        queue!(stdout, Hide)?;
    }
    end_synced_frame(stdout)?;
    let hit_frame = HitRect::from(frame);
    Ok(FormHitbox {
        frame: hit_frame,
        list: ListHitbox {
            frame: hit_frame,
            list: HitRect {
                x: inner_x,
                y: list_top,
                width: row_w as u16,
                height: visible_rows as u16,
            },
            start,
            total: fields.len(),
            visible_rows,
        },
        save: HitRect {
            x: inner_x,
            y: button_y,
            width: save_width,
            height: 1,
        },
        cancel: HitRect {
            x: cancel_x,
            y: button_y,
            width: cancel_width,
            height: 1,
        },
        footer,
    })
}

/// 【配置表单】【底部按钮绘制】绘制保存或取消按钮并返回其可见显示列宽。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `x`: 按钮起始列
/// - `y`: 按钮所在行
/// - `label`: 按钮文字
/// - `selected`: 是否获得键盘焦点
/// - `hovered`: 是否处于鼠标悬停态
/// - `primary`: 是否为主动作按钮
///
/// 返回:
/// - 按钮可见列宽
fn draw_form_button(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    label: &str,
    selected: bool,
    hovered: bool,
    primary: bool,
) -> Result<u16> {
    let palette = ThemePalette::current();
    queue!(stdout, MoveTo(x, y))?;
    let text = format!(" {label} ");
    if selected && primary {
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{}{}{BOLD}{text}{RESET}",
                    palette.button_bg, palette.button_fg
                ))
                .to_string()
            ))
        )?;
    } else if selected {
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!("{}{}{text}{RESET}", palette.select_bg, palette.accent)).to_string()
            ))
        )?;
    } else if hovered {
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!("{}{}{text}{RESET}", palette.hover_bg, palette.accent)).to_string()
            ))
        )?;
    } else {
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!("{}{text}{RESET}", palette.muted)).to_string()
            ))
        )?;
    }
    Ok(display_width(&text) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_tui::theme::SELECTION_BAR;

    /// 【配置表单】【连续悬停测试】验证非选中悬停行不画第二根异色竖条，且整行在复位后连续铺满悬停底色。
    #[test]
    fn hovered_form_row_keeps_continuous_hover_bg_without_second_bar() {
        let palette = ThemePalette::current();
        let field = Field::new("Protocol", "auto".to_string()).choices(&["auto", "openai-chat"]);
        let row = format_field_row(&field, 16, 60, false, false, true, false);

        assert!(!strip_field_ansi(&row).contains(SELECTION_BAR));
        let body = row.strip_suffix(RESET).unwrap();
        assert!(!body.contains(&format!("{RESET} ")));
        assert!(body.contains(&format!("{RESET}{}", palette.hover_bg)));
    }
}
