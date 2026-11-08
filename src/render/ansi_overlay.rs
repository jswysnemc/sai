//! 带样式终端行的列区间覆盖：浮层写入指定列，两侧保留底层文字与样式。

use crate::render::ansi_clip::ansi_visible_width;
use crate::render::ansi_style::update_active_sgr;
use crate::render::terminal_image::escape_sequence_end;
use unicode_width::UnicodeWidthChar;

/// 关闭 OSC 8 超链接的控制序列
const LINK_CLOSE: &str = "\x1b]8;;\x1b\\";

/// 【终端】【叠加绘制】把浮层文本写入底层行的列区间，左右两侧保留原内容和样式。
///
/// 被区间边界切开的宽字符替换为空格；区间右侧恢复底层当时的颜色与超链接，
/// 区间左侧未闭合的超链接在浮层前关闭，避免浮层文字变成链接。
///
/// 参数:
/// - `base`: 底层整行，可含 SGR 与其他控制序列
/// - `left`: 浮层首列
/// - `width`: 浮层占用列数
/// - `overlay`: 浮层文本，可见宽度不超过 `width`，不足部分以空格补齐
///
/// 返回:
/// - 合成后的整行；底层不足 `left` 列时用空格补齐
pub(crate) fn overlay_columns(base: &str, left: usize, width: usize, overlay: &str) -> String {
    let right = left + width;
    let mut output = String::with_capacity(base.len() + overlay.len() + 16);
    let mut active = String::new();
    let mut link = None;
    let mut column = 0usize;
    let mut tokens = tokens(base);
    let mut pending = None;
    // 1. 【终端】【左侧保留】复制浮层首列之前的内容，遇到首个越过首列的字符时停止
    for (token, cells) in tokens.by_ref() {
        match cells {
            None => {
                track(token, &mut active, &mut link);
                output.push_str(token);
            }
            Some(cells) if column + cells <= left => {
                output.push_str(token);
                column += cells;
            }
            Some(_) => {
                pending = Some((token, cells));
                break;
            }
        }
    }
    // 2. 【终端】【浮层写入】跨越首列的宽字符以空格补位，再关闭底层链接与样式
    output.push_str(&" ".repeat(left.saturating_sub(column)));
    if link.is_some() {
        output.push_str(LINK_CLOSE);
    }
    output.push_str("\x1b[0m");
    output.push_str(overlay);
    output.push_str("\x1b[0m");
    output.push_str(&" ".repeat(width.saturating_sub(ansi_visible_width(overlay))));
    // 3. 【终端】【右侧恢复】跳过被覆盖的列但继续追踪样式，跨越末列的宽字符以空格补位
    let mut tail = String::new();
    let mut resumed = false;
    for (token, cells) in pending.into_iter().chain(tokens) {
        match cells {
            None if column < right => track(token, &mut active, &mut link),
            None => tail.push_str(token),
            Some(0) if resumed => tail.push_str(token),
            Some(cells) if column >= right && cells > 0 => {
                tail.push_str(token);
                resumed = true;
            }
            Some(cells) if column < right && column + cells > right => {
                tail.push_str(&" ".repeat(column + cells - right));
                resumed = true;
            }
            Some(_) => {}
        }
        column += cells.unwrap_or(0);
    }
    if !tail.is_empty() {
        output.push_str(&active);
        output.push_str(link.as_deref().unwrap_or_default());
        output.push_str(&tail);
    }
    output
}

/// 【终端】【样式追踪】记录当前生效的 SGR 与未闭合超链接，供浮层右侧恢复。
/// 参数: token 为控制序列，active 为样式缓冲，link 为当前超链接开启序列；返回: 无。
fn track(token: &str, active: &mut String, link: &mut Option<String>) {
    if token.starts_with("\x1b[") && token.ends_with('m') {
        update_active_sgr(active, token);
    } else if let Some(rest) = token.strip_prefix("\x1b]8;") {
        let body = rest.trim_end_matches("\x1b\\").trim_end_matches('\x07');
        let uri = body.split_once(';').map_or("", |(_, uri)| uri);
        *link = (!uri.is_empty()).then(|| token.to_string());
    }
}

/// 【终端】【行切分】把行拆成控制序列与单个字符，字符附带显示列数，控制序列不占列。
/// 参数: text 为整行；返回: 片段与列数，控制序列的列数为空。
fn tokens(text: &str) -> impl Iterator<Item = (&str, Option<usize>)> {
    let mut index = 0usize;
    std::iter::from_fn(move || {
        let ch = text[index..].chars().next()?;
        let start = index;
        if ch == '\x1b' {
            index = escape_sequence_end(text, index)
                .max(index + 1)
                .min(text.len());
            return Some((&text[start..index], None));
        }
        index += ch.len_utf8();
        Some((&text[start..index], Some(ch.width().unwrap_or(0))))
    })
}

#[cfg(test)]
mod tests {
    use super::overlay_columns;
    use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};

    /// 【终端】【叠加回归】纯文本两侧保留原文字，总宽度不变。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_keeps_both_sides_of_plain_text() {
        let line = overlay_columns("abcdefghij", 2, 3, "XYZ");
        assert_eq!(strip_ansi(&line), "abXYZfghij");
        assert_eq!(ansi_visible_width(&line), 10);
    }

    /// 【终端】【样式恢复】区间右侧恢复底层被覆盖时生效的颜色，区间内样式不外溢。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_restores_base_style_after_region() {
        let line = overlay_columns("\x1b[31mabc\x1b[1mdef\x1b[0mgh", 1, 4, "\x1b[32mXXXX");
        assert_eq!(strip_ansi(&line), "aXXXXfgh");
        let (_, tail) = line.split_once("XXXX\x1b[0m").unwrap();
        assert!(tail.starts_with("\x1b[31m\x1b[1mf"), "{tail:?}");
    }

    /// 【终端】【宽字符边界】被首列或末列切开的中文以空格补位，列位置不偏移。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_replaces_split_wide_characters_with_spaces() {
        let line = overlay_columns("中文测试", 1, 4, "ABCD");
        assert_eq!(strip_ansi(&line), " ABCD 试");
        assert_eq!(ansi_visible_width(&line), 8);
    }

    /// 【终端】【短行补齐】底层不足首列时补空格，没有右侧内容时不追加样式。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_pads_short_base_rows() {
        let line = overlay_columns("\x1b[34mab", 4, 2, "XY");
        assert_eq!(strip_ansi(&line), "ab  XY");
        assert!(line.ends_with("XY\x1b[0m"));
    }

    /// 【终端】【超链接隔离】浮层前关闭底层链接，区间右侧重新打开同一链接。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_closes_and_reopens_hyperlinks() {
        let open = "\x1b]8;;https://example.invalid\x1b\\";
        let base = format!("{open}linktext\x1b]8;;\x1b\\ end");
        let line = overlay_columns(&base, 2, 2, "XY");
        assert_eq!(strip_ansi(&line), "liXYtext end");
        let (before, after) = line.split_once("XY").unwrap();
        assert!(before.ends_with("\x1b]8;;\x1b\\\x1b[0m"), "{before:?}");
        assert!(after.contains(&format!("{open}te")), "{after:?}");
    }
}
