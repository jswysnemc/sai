//! 为语法未分类的源码标识符补充主色，不改变空白、标点或已分类 token 的颜色。

use super::style::PRIMARY_STYLE;

/// 【代码着色】【普通标识符】分离未分类源码中的标识符与其他文本，保留原文及 UTF-8 边界。
/// 参数: `text` 为未分类片段，`emit` 接收颜色与对应原文；返回: 无。
pub(super) fn emit_spans(text: &str, mut emit: impl FnMut(&'static str, &str)) {
    let mut cursor = 0;
    let mut identifier_start = None;
    // 1. 【代码着色】【标识符识别】仅匹配字母或下划线开头、字母数字或下划线延续的标识符
    for (index, character) in text.char_indices() {
        if let Some(start) = identifier_start {
            if character != '_' && !character.is_alphanumeric() {
                emit(PRIMARY_STYLE, &text[start..index]);
                cursor = index;
                identifier_start = None;
            }
        } else if character == '_' || character.is_alphabetic() {
            if cursor < index {
                emit("", &text[cursor..index]);
            }
            identifier_start = Some(index);
        }
    }
    // 2. 【代码着色】【尾部保留】结束于标识符时补色，其他尾部保持默认样式
    if let Some(start) = identifier_start {
        emit(PRIMARY_STYLE, &text[start..]);
    } else if cursor < text.len() {
        emit("", &text[cursor..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【代码着色】【标识符边界】空白、运算符、纯数字及换行不着色，Unicode 标识符完整保留。
    /// 参数: 无；返回: 无。
    #[test]
    fn unscoped_identifiers_preserve_text_and_non_identifier_styles() {
        let source = "  _msg2 = 中文变量 + 42;\n";
        let mut spans = Vec::new();
        emit_spans(source, |style, text| spans.push((style, text.to_owned())));
        assert_eq!(
            spans,
            vec![
                ("", "  ".into()),
                (PRIMARY_STYLE, "_msg2".into()),
                ("", " = ".into()),
                (PRIMARY_STYLE, "中文变量".into()),
                ("", " + 42;\n".into()),
            ]
        );
        assert_eq!(
            spans
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<String>(),
            source
        );
    }

    /// 【代码着色】【片段边界】空片段不产生输出，尾部标识符仍完整着色。
    /// 参数: 无；返回: 无。
    #[test]
    fn empty_and_identifier_only_spans_are_handled() {
        let mut spans = Vec::new();
        emit_spans("", |style, text| spans.push((style, text.to_owned())));
        assert!(spans.is_empty());
        emit_spans("value_1", |style, text| {
            spans.push((style, text.to_owned()))
        });
        assert_eq!(spans, vec![(PRIMARY_STYLE, "value_1".into())]);
    }
}
