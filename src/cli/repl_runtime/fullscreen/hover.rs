//! 全屏正文的悬停高亮：鼠标停在可展开段落上时，把前景提亮一档。

use super::hover_color::{lifted_256, lifted_truecolor, HOVER_DEFAULT_FG};

const RESET: &str = "\x1b[0m";

/// 【全屏视图】【悬停高亮】给一行正文提亮前景，行内样式复位后重新补上。
///
/// 不铺底、不加粗。已有 256 色或真彩前景会向白色混合一档；
/// 没有前景的片段使用略低于纯白的默认色。
///
/// 参数:
/// - `line`: 已适配正文宽度的 ANSI 行
///
/// 返回:
/// - 带悬停提亮的行
pub(super) fn tint_line(line: &str) -> String {
    let mut output = String::with_capacity(line.len() + 64);
    output.push_str(HOVER_DEFAULT_FG);
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            output.push_str(&rewrite_sequence(&line[index..end]));
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

/// 【全屏视图】【样式改写】把 SGR 里的前景提亮；复位后补回默认悬停色。
///
/// 参数:
/// - `sequence`: 单个转义序列
///
/// 返回:
/// - 改写后的序列
fn rewrite_sequence(sequence: &str) -> String {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('m'))
    else {
        return sequence.to_string();
    };
    if params.is_empty() || params == "0" {
        return format!("{RESET}{HOVER_DEFAULT_FG}");
    }
    let parts: Vec<&str> = params.split(';').collect();
    let mut rewritten = Vec::new();
    let mut index = 0;
    let mut lifted_fg = false;
    while index < parts.len() {
        match parts[index] {
            "0" | "" => {
                rewritten.push("0".to_string());
                rewritten.push(hover_default_sgr());
                lifted_fg = true;
                index += 1;
            }
            "38" => match parts.get(index + 1) {
                Some(&"5") if parts.get(index + 2).is_some() => {
                    let code = parts[index + 2].parse::<u8>().unwrap_or(7);
                    rewritten.push(lifted_256(code));
                    lifted_fg = true;
                    index += 3;
                }
                Some(&"2") if parts.len() >= index + 5 => {
                    let red = parts[index + 2].parse::<u8>().unwrap_or(0);
                    let green = parts[index + 3].parse::<u8>().unwrap_or(0);
                    let blue = parts[index + 4].parse::<u8>().unwrap_or(0);
                    rewritten.push(lifted_truecolor(red, green, blue));
                    lifted_fg = true;
                    index += 5;
                }
                _ => {
                    rewritten.push(parts[index].to_string());
                    index += 1;
                }
            },
            "39" => {
                rewritten.push(hover_default_sgr());
                lifted_fg = true;
                index += 1;
            }
            "48" | "58" => {
                let skip = match parts.get(index + 1) {
                    Some(&"5") => 3,
                    Some(&"2") => 5,
                    _ => 1,
                };
                rewritten.extend(
                    parts[index..index + skip]
                        .iter()
                        .map(|item| (*item).to_string()),
                );
                index += skip;
            }
            other if ansi16_foreground(other).is_some() => {
                rewritten.push(lifted_256(ansi16_foreground(other).unwrap()));
                lifted_fg = true;
                index += 1;
            }
            other => {
                rewritten.push(other.to_string());
                index += 1;
            }
        }
    }
    if rewritten.is_empty() {
        return sequence.to_string();
    }
    let mut output = format!("\x1b[{}m", rewritten.join(";"));
    if !lifted_fg && resets_to_default_fg(&rewritten) {
        output.push_str(HOVER_DEFAULT_FG);
    }
    output
}

/// 默认悬停前景的 SGR 参数。
fn hover_default_sgr() -> String {
    HOVER_DEFAULT_FG
        .strip_prefix("\x1b[")
        .and_then(|rest| rest.strip_suffix('m'))
        .unwrap_or("38;5;250")
        .to_string()
}

/// 判断改写后的参数是否把前景恢复为默认。
fn resets_to_default_fg(parts: &[String]) -> bool {
    parts.iter().any(|part| part == "0" || part.is_empty())
}

/// 把 30–37 / 90–97 转成 256 色下标。
///
/// 参数:
/// - `code`: SGR 参数
///
/// 返回:
/// - 对应 256 色下标
fn ansi16_foreground(code: &str) -> Option<u8> {
    let value = code.parse::<u8>().ok()?;
    match value {
        30..=37 => Some(value - 30),
        90..=97 => Some(value - 90 + 8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 复位后补回默认悬停色，纯文本不变，不加粗不铺底。
    #[test]
    fn tint_survives_resets_and_keeps_text() {
        let output = tint_line("a\x1b[31mb\x1b[0mc");
        assert!(output.starts_with(HOVER_DEFAULT_FG));
        assert!(
            output.contains(&format!("{RESET}{HOVER_DEFAULT_FG}c")),
            "{output:?}"
        );
        assert!(output.ends_with(RESET));
        assert_eq!(plain(&output), "abc");
        assert!(!output.contains("\x1b[48;"));
        assert!(!output.contains("\x1b[1m"));
    }

    /// 256 色前景被改成更亮的真彩，颜色参数中的 0 不被当成复位。
    #[test]
    fn existing_foreground_is_lifted() {
        let output = tint_line("\x1b[38;5;244mdim");
        assert!(output.contains("38;2;"), "{output:?}");
        assert!(!output.contains("38;5;244"), "{output:?}");
        assert!(!output.contains("\x1b[1m"), "{output:?}");
        assert_eq!(plain(&output), "dim");
        assert!(!rewrite_sequence("\x1b[38;2;0;0;0m").contains("\x1b[0m"));
    }
}
