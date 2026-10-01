//! 全屏正文的悬停高亮：鼠标停在可展开段落上时，用浅色底标出整段可点击范围。

/// 悬停底色：比正文背景略亮一档，不压过 diff 与代码块自身的底色。
pub(super) const HOVER_BG: &str = "\x1b[48;5;236m";
const RESET: &str = "\x1b[0m";

/// 【全屏视图】【悬停高亮】给一行正文铺上悬停底色，行内样式复位后重新补上底色。
///
/// 正文自带背景色的片段（diff 增删行、代码块）保留原色，
/// 只有复位到默认背景的位置才补上悬停底色。
///
/// 参数:
/// - `line`: 已适配正文宽度的 ANSI 行
///
/// 返回:
/// - 带悬停底色的行
pub(super) fn tint_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len() + 32);
    output.push_str(HOVER_BG);
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            let sequence = &line[index..end];
            output.push_str(sequence);
            if resets_background(sequence) {
                output.push_str(HOVER_BG);
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

/// 【全屏视图】【样式解析】判断 SGR 序列是否把背景恢复为默认。
///
/// 参数:
/// - `sequence`: 单个转义序列
///
/// 返回:
/// - 含 `0`、空参数或 `49` 时为 true
fn resets_background(sequence: &str) -> bool {
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
            "0" | "" | "49" => return true,
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

    /// 复位序列之后补回悬停底色，纯文本不变。
    #[test]
    fn tint_survives_resets_and_keeps_text() {
        let output = tint_line("a\x1b[31mb\x1b[0mc");
        assert!(output.starts_with(HOVER_BG));
        assert!(
            output.contains(&format!("\x1b[0m{HOVER_BG}c")),
            "{output:?}"
        );
        assert!(output.ends_with(RESET));
        assert_eq!(plain(&output), "abc");
    }

    /// 自带背景色的片段保留原色，颜色参数中的 0 不算复位。
    #[test]
    fn explicit_colors_are_not_treated_as_resets() {
        assert!(!resets_background("\x1b[48;5;22m"));
        assert!(!resets_background("\x1b[38;2;0;0;0m"));
        assert!(!resets_background("\x1b[1;31m"));
        assert!(resets_background("\x1b[m"));
        assert!(resets_background("\x1b[49m"));
        assert!(resets_background("\x1b[2;0m"));
        assert!(!resets_background("\x1b[2K"));
    }
}
