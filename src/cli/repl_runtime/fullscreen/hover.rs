//! 全屏正文的悬停高亮：鼠标停在可展开段落上时，用加粗前景标出整段可点击范围。

/// 悬停加粗：不铺底色，避免盖住代码块与 diff 自身的背景。
pub(super) const HOVER_STYLE: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// 【全屏视图】【悬停高亮】给一行正文加上加粗前景，行内样式复位后重新补上。
///
/// 参数:
/// - `line`: 已适配正文宽度的 ANSI 行
///
/// 返回:
/// - 带悬停加粗的行
pub(super) fn tint_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len() + 32);
    output.push_str(HOVER_STYLE);
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            let sequence = &line[index..end];
            output.push_str(sequence);
            if resets_hover_style(sequence) {
                output.push_str(HOVER_STYLE);
            }
            index = end;
            continue;
        }
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        output.push(ch);
        index += ch.len_utf8();
    }
    output.push_str(RESET);
    output
}

/// 【全屏视图】【样式解析】判断 SGR 序列是否把加粗恢复为默认。
///
/// 参数:
/// - `sequence`: 单个转义序列
///
/// 返回:
/// - 含 `0`、空参数或 `22` 时为 true
fn resets_hover_style(sequence: &str) -> bool {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('m'))
    else {
        return false;
    };
    if params.is_empty() {
        return true;
    }
    // 跳过 38;5;N / 38;2;R;G;B 这类前景色参数，避免把颜色值误判为复位
    let parts: Vec<&str> = params.split(';').collect();
    let mut index = 0;
    while index < parts.len() {
        match parts[index] {
            "0" | "" | "22" => return true,
            "38" | "48" | "58" => {
                index += match parts.get(index + 1) {
                    Some(&"5") => 3,
                    Some(&"2") => 5,
                    _ => 1,
                };
            }
            _ => index += 1,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 复位序列之后补回悬停加粗，纯文本不变。
    #[test]
    fn tint_survives_resets_and_keeps_text() {
        let output = tint_line("a\x1b[31mb\x1b[0mc");
        assert!(output.starts_with(HOVER_STYLE));
        assert!(
            output.contains(&format!("\x1b[0m{HOVER_STYLE}c")),
            "{output:?}"
        );
        assert!(output.ends_with(RESET));
        assert_eq!(plain(&output), "abc");
        assert!(!output.contains("\x1b[48;"));
    }

    /// 颜色参数中的 0 不算复位；真正的加粗复位要识别。
    #[test]
    fn explicit_colors_are_not_treated_as_resets() {
        assert!(!resets_hover_style("\x1b[48;5;22m"));
        assert!(!resets_hover_style("\x1b[38;2;0;0;0m"));
        assert!(!resets_hover_style("\x1b[1;31m"));
        assert!(resets_hover_style("\x1b[m"));
        assert!(resets_hover_style("\x1b[22m"));
        assert!(resets_hover_style("\x1b[2;0m"));
        assert!(!resets_hover_style("\x1b[2K"));
    }
}
