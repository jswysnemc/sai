//! 表单字段值展示、密钥脱敏、宽屏右侧详情栏与窄屏底部单行说明摘要。

use super::super::theme::{ThemePalette, BOLD, RESET};
use super::super::ui::{pad, truncate};
use super::{choice_label, parse_bool_field, Field};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io;

/// 掩码星号上限，超出后以省略号收尾，防止泄露密钥真实长度。
const MASK_SECRET_MAX_STARS: usize = 8;

/// 【配置表单】【清理ANSI】剥离字段展示文本中的 ANSI 转义序列以便计算纯文本列宽与选中态文本。
///
/// 参数:
/// - `value`: 含 ANSI 样式的字段文本
///
/// 返回:
/// - 不含转义序列的纯文本
pub(super) fn strip_field_ansi(value: &str) -> String {
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

/// 【配置表单】【右侧说明绘制】在宽终端右侧栏绘制当前聚焦字段的多行说明与可选值预览。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `x`: 说明栏起始列
/// - `y`: 说明栏起始行
/// - `width`: 说明栏列宽
/// - `height`: 说明栏行数
/// - `text`: 说明正文
///
/// 返回:
/// - 绘制结果
pub(super) fn draw_form_detail(
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
    let palette = ThemePalette::current();
    queue!(
        stdout,
        MoveTo(x, y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{BOLD}◈ {}{RESET}",
                palette.brand,
                truncate(t("Field", "字段"), width.saturating_sub(2) as usize)
            ))
            .to_string()
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
            Print(crate::render::readable_terminal_text(
                &(format!("{}{}{RESET}", palette.muted, pad(&line, width as usize))).to_string()
            ))
        )?;
    }
    Ok(())
}

/// 【配置表单】【完整说明生成】按字段类型生成宽终端右侧栏的多段详细说明。
///
/// 参数:
/// - `field`: 当前聚焦的表单字段
///
/// 返回:
/// - 多段落说明文本
pub(super) fn field_detail_text(field: &Field) -> String {
    let mut parts = vec![field.label.clone()];
    if field.readonly {
        parts.push(
            t(
                "Read-only: fixed identifier, cannot be edited after creation.",
                "只读字段：固定配置标识，创建后不可修改。",
            )
            .to_string(),
        );
    } else if field.boolean {
        parts.push(
            t(
                "Toggle: Space, ←/→ or Enter flips on/off.",
                "开关：Space、←/→ 或 Enter 直接切换开关。",
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
        let hint = if field.choices.len() <= 5 {
            t(
                "Choice: Space, ←/→ or Enter cycles inline.",
                "选项：Space、←/→ 或 Enter 直接行内切换。",
            )
        } else {
            t(
                "Choice list: Space/←/→ cycles or Enter opens picker.",
                "选项列表：Space/←/→ 快速切换，Enter 打开选择器。",
            )
        };
        let options_preview = field
            .choices
            .iter()
            .take(6)
            .map(|item| choice_label(item, field.empty_choice_label))
            .collect::<Vec<_>>()
            .join(" · ");
        parts.push(format!(
            "{hint}\n\n{}: {options_preview}",
            t("Options", "可选")
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
                "Text: Enter to edit, Tab moves next, Ctrl+S saves immediately.",
                "文本：Enter 编辑，Tab 切换下一项，Ctrl+S 直接保存。",
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

/// 【配置表单】【窄屏紧凑摘要】在 80 列等隐藏右侧栏的窄终端中，为底部摘要行生成单行操作提示。
///
/// 参数:
/// - `fields`: 表单字段切片
/// - `selected`: 当前焦点下标（含底部按钮）
///
/// 返回:
/// - 单行紧凑说明摘要
pub(super) fn compact_focus_hint(fields: &[Field], selected: usize) -> String {
    if selected == fields.len() {
        return t(
            "Save writes edits to in-memory config (persist from main menu).",
            "保存：写入当前内存配置（主菜单保存退出后落盘）。",
        )
        .to_string();
    }
    if selected > fields.len() {
        return t(
            "Cancel discards edits in this form and returns.",
            "取消：丢弃当前表单改动并返回上一屏。",
        )
        .to_string();
    }
    let field = &fields[selected];
    if field.readonly {
        t(
            "Read-only identifier · Tab/↑↓ to move",
            "只读标识，不可修改 · Tab/↑↓ 切换字段",
        )
        .to_string()
    } else if field.boolean {
        t(
            "Toggle · Space / ←→ / Enter flips on or off",
            "开关 · Space / ←→ / Enter 直接切换",
        )
        .to_string()
    } else if field.textarea {
        t(
            "Multiline · Enter opens $EDITOR external editor",
            "多行文本 · Enter 打开 $EDITOR 外部编辑器",
        )
        .to_string()
    } else if !field.choices.is_empty() {
        let preview = field
            .choices
            .iter()
            .take(4)
            .map(|item| choice_label(item, field.empty_choice_label))
            .collect::<Vec<_>>()
            .join(" · ");
        format!(
            "{} · {}: {preview}",
            t("Choice: Space/←→ cycle", "选项：Space/←→ 切换"),
            t("Options", "可选")
        )
    } else if field.secret {
        t(
            "Secret · Enter edit, Ctrl+R toggle plain text",
            "密钥 · Enter 编辑，编辑时 Ctrl+R 切换明文",
        )
        .to_string()
    } else {
        t(
            "Text · Enter edit, Tab next, s/Ctrl+S save",
            "文本 · Enter 编辑，Tab 下一项，s/Ctrl+S 保存",
        )
        .to_string()
    }
}

/// 【配置表单】【字段值渲染】按字段语义（只读、开关、多行、枚举、密钥、普通文本）生成带样式的展示文本。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否明文显示密钥
/// - `max_value_width`: 值列可用显示宽度
///
/// 返回:
/// - 已样式化的单行字段值文本
pub(super) fn field_display_value(
    field: &Field,
    revealed_secret: bool,
    max_value_width: usize,
) -> String {
    let palette = ThemePalette::current();
    let vdim = palette.dim;
    let ok = palette.ok;
    let val_color = palette.value;
    if field.readonly {
        // 1. 【配置表单】【只读渲染】只读字段以静态无箭头文本展示，消除伪装可切换枚举的误导
        let raw = if field.value.is_empty() {
            t("(empty)", "（空）").to_string()
        } else {
            rendered_text_value(field, revealed_secret)
        };
        format!(
            "{}{}{RESET}",
            palette.muted,
            truncate(&raw, max_value_width)
        )
    } else if field.boolean {
        match parse_bool_field(&field.value) {
            Ok(true) => format!("{ok}● {}{RESET}", t("on", "启用")),
            Ok(false) => format!("{vdim}○ {}{RESET}", t("off", "关闭")),
            Err(_) => rendered_text_value(field, revealed_secret),
        }
    } else if field.textarea && field.value.is_empty() {
        format!(
            "{vdim}{}{RESET}",
            t("(Enter opens $EDITOR)", "(Enter 打开 $EDITOR)")
        )
    } else if !field.choices.is_empty() {
        let label = if field.value.is_empty() {
            field.empty_choice_label.to_string()
        } else {
            choice_label(&field.value, field.empty_choice_label)
        };
        if field.choices.len() <= 5 {
            format!(
                "{vdim}‹ {RESET}{val_color}{}{RESET}{vdim} ›{RESET}",
                truncate(&label, max_value_width.saturating_sub(4)),
            )
        } else {
            format!(
                "{val_color}{}{RESET} {vdim}▾{RESET}",
                truncate(&label, max_value_width.saturating_sub(2)),
            )
        }
    } else if field.value.is_empty() {
        format!("{vdim}{}{RESET}", t("(empty)", "（空）"))
    } else {
        format!(
            "{val_color}{}{RESET}",
            truncate(
                &rendered_text_value(field, revealed_secret),
                max_value_width,
            )
        )
    }
}

/// 【配置表单】【文本值展开】折叠换行符并按密钥开关决定是否执行星号脱敏。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否允许显示明文
///
/// 返回:
/// - 单行纯文本值
pub(super) fn rendered_text_value(field: &Field, revealed_secret: bool) -> String {
    let value = field.value.replace('\n', " ");
    if field.secret && !revealed_secret {
        mask_secret(&value)
    } else {
        value
    }
}

/// 【配置表单】【密钥脱敏】按固定星号上限对密钥文本打码，防止暴露长密钥位数特征。
///
/// 参数:
/// - `value`: 原始密钥文本
///
/// 返回:
/// - 脱敏后的星号文本
pub(super) fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.chars().count() <= MASK_SECRET_MAX_STARS {
        return "*".repeat(value.chars().count());
    }
    format!("{}…", "*".repeat(MASK_SECRET_MAX_STARS))
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

        assert_eq!(
            strip_field_ansi(&field_display_value(&field, false, 70)),
            "********…"
        );
    }

    #[test]
    fn mask_secret_keeps_short_values_verbatim() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("abc"), "***");
        assert_eq!(mask_secret(&"x".repeat(MASK_SECRET_MAX_STARS)), "********");
    }

    #[test]
    fn boolean_field_renders_status_dot() {
        let on = Field::boolean("Tools", true);
        let off = Field::boolean("Tools", false);

        assert!(strip_field_ansi(&field_display_value(&on, false, 70)).starts_with('●'));
        assert!(strip_field_ansi(&field_display_value(&off, false, 70)).starts_with('○'));
    }

    /// 【配置表单】【只读展示测试】验证只读字段输出纯净静态文本，不再出现伪装枚举切换的尖括号箭头。
    #[test]
    fn readonly_field_renders_plain_static_text_without_choice_arrows() {
        let field = Field::new("Config ID", "openai".to_string()).readonly();
        let plain = strip_field_ansi(&field_display_value(&field, false, 40));

        assert_eq!(plain, "openai");
        assert!(!plain.contains('‹'));
        assert!(!plain.contains('›'));
    }
}
