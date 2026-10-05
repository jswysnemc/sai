//! CommonMark 风格的行内代码 span：按围栏长度配对，未闭合时按字面量输出。

/// 【终端】【行内代码】按相同长度的反引号围栏取出代码内容。
///
/// 起始处连续 N 个反引号必须匹配同样长度的结束围栏。内容两端都有空格且
/// 不全是空白时，各剥掉一个空格。找不到闭合围栏时返回空，调用方按字面量输出。
///
/// 参数:
/// - `chars`: 原始字符数组
/// - `start`: 开围栏第一个反引号的下标
///
/// 返回:
/// - `(内容, 结束围栏之后的下标)`；未闭合时为空
pub(super) fn take_code_span(chars: &[char], start: usize) -> Option<(String, usize)> {
    let open = run_len(chars, start, '`');
    if open == 0 {
        return None;
    }
    let content_start = start + open;
    let mut index = content_start;
    while index < chars.len() {
        let close = run_len(chars, index, '`');
        if close == open {
            let mut content: String = chars[content_start..index].iter().collect();
            // 1. 两端都有空格且内容不全是空白时，各剥一个空格
            if content.len() >= 2
                && content.starts_with(' ')
                && content.ends_with(' ')
                && content.chars().any(|ch| ch != ' ')
            {
                content.pop();
                content.remove(0);
            }
            return Some((content, index + close));
        }
        index += close.max(1);
    }
    None
}

/// 从 `start` 起连续相同字符的个数。
///
/// 参数:
/// - `chars`: 字符数组
/// - `start`: 起始下标
/// - `marker`: 目标字符
///
/// 返回:
/// - 连续个数
fn run_len(chars: &[char], start: usize, marker: char) -> usize {
    chars[start..]
        .iter()
        .take_while(|ch| **ch == marker)
        .count()
}

/// 【终端】【行内样式】给已渲染的内层文本套外层样式，内层复位后补回外层。
///
/// 参数:
/// - `style`: 开样式序列
/// - `inner`: 已渲染的内层文本
/// - `reset`: 复位序列
///
/// 返回:
/// - 套好外层样式的文本
pub(super) fn wrap_inline_style(style: &str, inner: &str, reset: &str) -> String {
    let mut output = String::with_capacity(style.len() + inner.len() + reset.len() + 8);
    output.push_str(style);
    let mut index = 0usize;
    while index < inner.len() {
        if inner.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(inner, index)
                .max(index + 1)
                .min(inner.len());
            let sequence = &inner[index..end];
            output.push_str(sequence);
            if sequence == reset {
                output.push_str(style);
            }
            index = end;
            continue;
        }
        let Some(ch) = inner[index..].chars().next() else {
            break;
        };
        output.push(ch);
        index += ch.len_utf8();
    }
    output.push_str(reset);
    output
}
