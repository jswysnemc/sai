use super::{choice_label, parse_bool_field, take_chars, Field};
use crate::config_tui::layout::{form_column_widths, form_label_width, full_frame, scroll_start};
use crate::config_tui::theme::{selection_marks, ACCENT, BOLD, BRAND, DIM, MUTED, RESET};
use crate::config_tui::ui::{display_width, draw_box, pad, truncate};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

/// 【配置表单】【终端绘制】按窗口尺寸和当前编辑状态显示字段及操作按钮
/// @param stdout 终端输出；title 为标题；fields 为字段；selected 为选中位置；editing 为编辑状态；cursors 为光标；revealed_secrets 为显示偏好
/// @returns 终端绘制结果
pub(super) fn draw_form(
    stdout: &mut io::Stdout,
    title: &str,
    fields: &[Field],
    selected: usize,
    editing: bool,
    cursors: &[usize],
    revealed_secrets: &[bool],
) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let frame = full_frame(cols, rows);
    let x = frame.x;
    let y = frame.y;
    let width = frame.width;
    let height = frame.height;

    super::super::ui::begin_synced_frame(stdout)?;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, title)?;
    let inner_x = x.saturating_add(2);
    let inner_w = width.saturating_sub(4);
    // 字段区占满中间；底两行留给按钮与状态；宽终端右侧放字段说明
    let list_top = y.saturating_add(2);
    let list_bottom = y.saturating_add(height.saturating_sub(3));
    let body_h = list_bottom.saturating_sub(list_top).max(1);
    let (left_w, right_w) = form_column_widths(inner_w);
    // 标签列对齐成一条竖线，但宽度让位于值列，避免 URL / 密钥被挤成省略号
    let longest_label = fields
        .iter()
        .filter(|field| !field.section)
        .map(|field| display_width(field.label))
        .max()
        .unwrap_or(8);
    let label_col = form_label_width(longest_label, left_w as usize);
    let visible_rows = body_h as usize;
    let start = scroll_start(selected.min(fields.len().saturating_sub(1)), visible_rows);
    let mut cursor = None;
    for row in 0..visible_rows {
        let index = start + row;
        let row_y = list_top.saturating_add(row as u16);
        queue!(stdout, MoveTo(inner_x, row_y))?;
        if index >= fields.len() {
            queue!(stdout, Print(" ".repeat(left_w as usize)))?;
            continue;
        }
        let field = &fields[index];
        if field.section {
            // 分组标题：品牌色短横 + 名称 + 弱化延伸线
            let name = truncate(field.label, left_w.saturating_sub(6) as usize);
            let used = 4 + display_width(&name) + 1;
            let tail = (left_w as usize).saturating_sub(used);
            queue!(
                stdout,
                Print(format!(
                    "{DIM}── {RESET}{BRAND}{BOLD}{name}{RESET} {DIM}{}{RESET}",
                    "─".repeat(tail)
                ))
            )?;
            continue;
        }
        let is_selected = index == selected && !editing;
        let is_editing_row = index == selected && editing;
        let (bar, row_style) = selection_marks(is_selected);
        let label_text = pad(field.label, label_col);
        let value_budget = (left_w as usize).saturating_sub(2 + label_col + 2).max(8);
        let value = field_display_value(field, revealed_secrets[index], value_budget);
        if is_selected {
            // 选中行：整行深底统一亮青，焦点清晰优先于分色
            let line = truncate(
                &format!("{label_text}  {}", strip_field_ansi(&value)),
                left_w.saturating_sub(2) as usize,
            );
            queue!(
                stdout,
                Print(format!(
                    "{bar}{row_style} {}{RESET}",
                    pad(&line, left_w.saturating_sub(2) as usize)
                ))
            )?;
        } else {
            // 常规行：标签弱化、值走语义色，两列对齐
            let editing_marker = if is_editing_row {
                format!("{ACCENT}▸{RESET}")
            } else {
                bar
            };
            queue!(
                stdout,
                Print(format!(
                    "{editing_marker} {MUTED}{label_text}{RESET}  {value}"
                ))
            )?;
            let used = 2 + label_col + 2 + display_width(&strip_field_ansi(&value));
            let remaining = (left_w as usize).saturating_sub(used);
            if remaining > 0 {
                queue!(stdout, Print(" ".repeat(remaining)))?;
            }
        }
        if is_editing_row {
            let rendered_value = rendered_text_value(field, revealed_secrets[index]);
            let cursor_text = take_chars(&rendered_value, cursors[index]);
            let cursor_x = inner_x
                + 2
                + label_col as u16
                + 2
                + display_width(&truncate(&cursor_text, value_budget)) as u16;
            cursor = Some((
                cursor_x.min(inner_x.saturating_add(left_w.saturating_sub(1))),
                row_y,
            ));
        }
    }
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
    }
    let button_y = y.saturating_add(height.saturating_sub(2));
    let save_width = draw_form_button(
        stdout,
        inner_x,
        button_y,
        t("Save", "保存"),
        selected == fields.len() && !editing,
        true,
    )?;
    draw_form_button(
        stdout,
        inner_x.saturating_add(save_width).saturating_add(2),
        button_y,
        t("Cancel", "取消"),
        selected == fields.len() + 1 && !editing,
        false,
    )?;

    // 底部帮助条按当前状态切换内容
    let help = if selected < fields.len() && editing && fields[selected].secret {
        super::super::theme::help_line(&[
            ("Enter/Esc", t("done", "结束")),
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
            ("←→", t("cursor", "光标")),
            ("Home/End", t("jump", "行首尾")),
        ])
    } else {
        super::super::theme::help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("edit", "编辑")),
            ("s", t("save", "保存")),
            ("q", t("cancel", "取消")),
        ])
    };
    super::super::ui::draw_status_bar(stdout, &frame, &help)?;
    // 光标显隐放在同步帧内，避免先闪出空屏再出现光标
    if let Some((cx, cy)) = cursor {
        queue!(stdout, Show, MoveTo(cx, cy))?;
    } else {
        queue!(stdout, Hide)?;
    }
    super::super::ui::end_synced_frame(stdout)?;
    Ok(())
}

/// 去除字段值渲染中的 ANSI 序列（选中行统一样式时使用）。
fn strip_field_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut index = 0usize;
    while index < value.len() {
        if value[index..].starts_with('\x1b') {
            index = crate::render::terminal_image::escape_sequence_end(value, index).max(index + 1);
            continue;
        }
        let ch = value[index..].chars().next().unwrap_or_default();
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

/// 表单右侧字段说明栏。
fn draw_form_detail(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    text: &str,
) -> Result<()> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    queue!(
        stdout,
        MoveTo(x, y),
        Print(format!(
            "{BRAND}{BOLD}◈ {}{RESET}",
            truncate(t("Field", "字段"), width.saturating_sub(2) as usize)
        ))
    )?;
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    let max_w = width as usize;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            current_w = 0;
            continue;
        }
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_w + w > max_w && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_w = 0;
        }
        current.push(ch);
        current_w += w;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    for (row, line) in lines
        .into_iter()
        .take(height.saturating_sub(2) as usize)
        .enumerate()
    {
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16).saturating_add(2)),
            Print(format!("{MUTED}{}{RESET}", pad(&line, width as usize)))
        )?;
    }
    Ok(())
}

/// 根据字段类型生成右侧说明。
fn field_detail_text(field: &Field) -> String {
    let mut parts = vec![field.label.to_string()];
    if field.boolean {
        parts.push(
            t(
                "Toggle: Enter flips true/false.",
                "开关：Enter 在 true/false 间切换。",
            )
            .to_string(),
        );
    } else if field.textarea {
        parts.push(
            t(
                "Multiline: Enter opens $EDITOR, then returns here.",
                "多行：Enter 打开 $EDITOR，保存后回到此屏。",
            )
            .to_string(),
        );
    } else if !field.choices.is_empty() {
        parts.push(format!(
            "{} ({})",
            t(
                "Choice list: Enter opens a picker.",
                "选项列表：Enter 打开选择器。"
            ),
            field.choices.len()
        ));
    } else if field.secret {
        parts.push(
            t(
                "Secret: shown masked; Ctrl+R toggles plain text while editing.",
                "密钥：默认掩码；编辑时 Ctrl+R 切换明文。",
            )
            .to_string(),
        );
    } else {
        parts.push(
            t(
                "Text: Enter to edit, type freely, Enter/Esc to finish.",
                "文本：Enter 编辑，输入后 Enter/Esc 结束。",
            )
            .to_string(),
        );
    }
    if !field.value.trim().is_empty() && !field.secret {
        let preview = truncate(&field.value.replace('\n', " "), 80);
        parts.push(format!("{}: {preview}", t("Current", "当前值")));
    }
    parts.join("\n\n")
}

/// 返回表单字段展示文本。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否显示密钥明文
/// - `max_value_width`: 值区域可用显示宽度
///
/// 返回:
/// - 字段展示文本
fn field_display_value(field: &Field, revealed_secret: bool, max_value_width: usize) -> String {
    use super::super::theme::{DIM as VDIM, OK, VALUE};
    if field.boolean {
        match parse_bool_field(&field.value) {
            Ok(true) => format!("{OK}● {}{RESET}", t("on", "启用")),
            Ok(false) => format!("{VDIM}○ {}{RESET}", t("off", "关闭")),
            Err(_) => rendered_text_value(field, revealed_secret),
        }
    } else if field.textarea && field.value.is_empty() {
        format!(
            "{VDIM}{}{RESET}",
            t("(Enter opens $EDITOR)", "(Enter 打开 $EDITOR)")
        )
    } else if !field.choices.is_empty() && field.value.is_empty() {
        format!(
            "{VALUE}{}{RESET} {VDIM}▾{RESET}",
            truncate(field.empty_choice_label, max_value_width.saturating_sub(2))
        )
    } else if !field.choices.is_empty() {
        format!(
            "{VALUE}{}{RESET} {VDIM}▾{RESET}",
            truncate(
                &choice_label(&field.value, field.empty_choice_label),
                max_value_width.saturating_sub(2),
            )
        )
    } else if field.value.is_empty() {
        format!("{VDIM}{}{RESET}", t("(empty)", "（空）"))
    } else {
        format!(
            "{VALUE}{}{RESET}",
            truncate(
                &rendered_text_value(field, revealed_secret),
                max_value_width,
            )
        )
    }
}

/// 返回单行文本字段渲染值。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否显示密钥明文
///
/// 返回:
/// - 字段单行渲染值
fn rendered_text_value(field: &Field, revealed_secret: bool) -> String {
    let value = field.value.replace('\n', " ");
    if field.secret && !revealed_secret {
        mask_secret(&value)
    } else {
        value
    }
}

/// 掩码密钥文本。
///
/// 定宽掩码：不按长度打星，避免从星号数量反推出密钥长度
/// （32 位 hex、108 位 OpenAI key、sk-ant-… 会立刻被区分开）。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 掩码后文本
fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.chars().count() <= MASK_SECRET_MAX_STARS {
        return "*".repeat(value.chars().count());
    }
    format!("{}…", "*".repeat(MASK_SECRET_MAX_STARS))
}

/// 掩码星号上限，超出后以省略号收尾。
const MASK_SECRET_MAX_STARS: usize = 8;

/// 绘制表单按钮，返回按钮可见宽度（供横向排布）。
///
/// 主按钮选中时为品牌实心底，次按钮选中时为深底描边感；
/// 未选中一律弱化，视觉焦点始终只有一个。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `x`: 起始列
/// - `y`: 所在行
/// - `label`: 按钮文字
/// - `selected`: 是否选中
/// - `primary`: 是否主按钮
///
/// 返回:
/// - 按钮渲染后的可见宽度
fn draw_form_button(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    label: &str,
    selected: bool,
    primary: bool,
) -> Result<u16> {
    use super::super::theme::{BUTTON_BG, BUTTON_FG, SELECT_BG};
    queue!(stdout, MoveTo(x, y))?;
    let text = format!(" {label} ");
    if selected && primary {
        queue!(
            stdout,
            Print(format!("{BUTTON_BG}{BUTTON_FG}{BOLD}{text}{RESET}"))
        )?;
    } else if selected {
        queue!(stdout, Print(format!("{SELECT_BG}{ACCENT}{text}{RESET}")))?;
    } else {
        queue!(stdout, Print(format!("{MUTED}{text}{RESET}")))?;
    }
    Ok(display_width(&text) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_field_is_masked_by_default() {
        let field = Field::new("Token", "secret".to_string()).secret();

        assert_eq!(
            strip_field_ansi(&field_display_value(&field, false, 70)),
            "******"
        );
    }

    #[test]
    fn secret_field_can_be_revealed() {
        let field = Field::new("Token", "secret".to_string()).secret();

        assert_eq!(
            strip_field_ansi(&field_display_value(&field, true, 70)),
            "secret"
        );
    }

    #[test]
    fn secret_textarea_is_masked_by_default() {
        let field = Field::textarea("Tokens", "first\nsecond".to_string()).secret();

        // 定宽掩码：不泄露真实长度
        assert_eq!(
            strip_field_ansi(&field_display_value(&field, false, 70)),
            "********…"
        );
    }

    /// 短密钥按原长度打星，空值不产生掩码。
    #[test]
    fn mask_secret_keeps_short_values_verbatim() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("abc"), "***");
        assert_eq!(mask_secret(&"x".repeat(MASK_SECRET_MAX_STARS)), "********");
    }

    /// 布尔字段渲染为状态圆点而不是原始 true/false。
    #[test]
    fn boolean_field_renders_status_dot() {
        let on = Field::boolean("Tools", true);
        let off = Field::boolean("Tools", false);

        assert!(strip_field_ansi(&field_display_value(&on, false, 70)).starts_with('●'));
        assert!(strip_field_ansi(&field_display_value(&off, false, 70)).starts_with('○'));
    }
}
