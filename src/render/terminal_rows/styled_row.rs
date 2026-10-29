//! 按字符簇解析终端行，保留安全追加所需的字节位置和 SGR 样式。

use unicode_segmentation::UnicodeSegmentation;

pub(super) struct Segment<'a> {
    pub text: &'a str,
    pub style: String,
    pub start: usize,
    pub width: usize,
}

/// 可覆盖的新增字符范围；末端不包含未变化的填充空格和右侧轨道。
pub(super) struct AppendSpan {
    pub column: usize,
    pub first: usize,
    pub end: usize,
}

/// 【终端】【行解析】识别零宽 SGR，样式和字符簇保留原始文本切片。
/// 参数：text 为带 ANSI 的完整行；返回：可安全比较的字符簇，控制字符或跨样式字符簇返回空。
pub(super) fn parse(text: &str) -> Option<Vec<Segment<'_>>> {
    let mut segments = Vec::new();
    let mut cursor = 0;
    let mut style = String::new();
    let mut visible = String::new();
    while cursor < text.len() {
        if text[cursor..].starts_with('\x1b') {
            let end = crate::render::terminal_image::escape_sequence_end(text, cursor);
            let sequence = text.get(cursor..end)?;
            let parameters = sequence.strip_prefix("\x1b[")?.strip_suffix('m')?;
            if end <= cursor
                || !parameters
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
            {
                return None;
            }
            if parameters.is_empty() || parameters == "0" {
                style.clear();
            } else {
                style.push_str(sequence);
            }
            cursor = end;
            continue;
        }
        let end = text[cursor..]
            .find('\x1b')
            .map_or(text.len(), |offset| cursor + offset);
        for (offset, grapheme) in UnicodeSegmentation::grapheme_indices(&text[cursor..end], true) {
            let width = unicode_width::UnicodeWidthStr::width(grapheme);
            if width == 0 || grapheme.chars().any(char::is_control) {
                return None;
            }
            visible.push_str(grapheme);
            segments.push(Segment {
                text: grapheme,
                style: style.clone(),
                start: cursor + offset,
                width,
            });
        }
        cursor = end;
    }
    // 1. 【终端】【行解析】ANSI 分隔可能拆开组合字符，只有分段与可见字符簇完全一致才允许追加
    let graphemes = UnicodeSegmentation::graphemes(visible.as_str(), true).collect::<Vec<_>>();
    if graphemes.len() != segments.len()
        || graphemes
            .iter()
            .zip(&segments)
            .any(|(text, segment)| *text != segment.text)
    {
        return None;
    }
    Some(segments)
}

/// 【终端】【安全追加】稳定前缀后只能追加，或用新增文字替换填充空格，右侧轨道必须保持原列。
/// 参数：old/new 为新旧行字符簇；返回：允许绘制的新增范围，其他改写返回空。
pub(super) fn append_span(old: &[Segment<'_>], new: &[Segment<'_>]) -> Option<AppendSpan> {
    let equal = |a: &Segment<'_>, b: &Segment<'_>| {
        a.text == b.text && a.style == b.style && a.width == b.width
    };
    let first = old.iter().zip(new).take_while(|(a, b)| equal(a, b)).count();
    let column = new[..first].iter().map(|segment| segment.width).sum();
    if first == new.len() {
        return None;
    }
    if first == old.len() {
        return Some(AppendSpan {
            column,
            first,
            end: new.len(),
        });
    }
    // 1. 【终端】【填充追加】全屏行补齐到固定宽度，新增正文只替换空格而不移动概览或滚动条
    let old_width = old.iter().map(|segment| segment.width).sum::<usize>();
    let new_width = new.iter().map(|segment| segment.width).sum::<usize>();
    if old_width != new_width {
        return None;
    }
    let suffix = old[first..]
        .iter()
        .rev()
        .zip(new[first..].iter().rev())
        .take_while(|(a, b)| equal(a, b))
        .count();
    let end = new.len() - suffix;
    if end <= first
        || old[first..old.len() - suffix]
            .iter()
            .any(|segment| segment.text != " ")
    {
        return None;
    }
    Some(AppendSpan { column, first, end })
}
