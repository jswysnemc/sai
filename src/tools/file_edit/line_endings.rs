/// 文件的行尾风格。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum LineEndingStyle {
    /// 全部使用 \n
    Lf,
    /// 全部使用 \r\n
    Crlf,
    /// 混用，或存在孤立的 \r
    Mixed,
}

/// 面向模型展示的文本视图。
#[derive(Debug, Clone)]
pub(crate) struct ModelTextView {
    /// 已归一化为 LF 的文本
    pub(crate) text: String,
    /// 原文件的行尾风格，写回时据此还原
    pub(crate) line_ending_style: LineEndingStyle,
}

/// 探测文本的行尾风格。
///
/// 参数:
/// - `text`: 原始文件内容
///
/// 返回:
/// - 行尾风格；混用或含孤立 \r 时为 Mixed
pub(crate) fn detect_line_ending_style(text: &str) -> LineEndingStyle {
    let bytes = text.as_bytes();
    let mut has_crlf = false;
    let mut has_lf = false;
    let mut has_lone_cr = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    has_crlf = true;
                    index += 1;
                } else {
                    has_lone_cr = true;
                }
            }
            b'\n' => has_lf = true,
            _ => {}
        }
        index += 1;
    }
    if has_lone_cr || (has_crlf && has_lf) {
        return LineEndingStyle::Mixed;
    }
    if has_crlf {
        return LineEndingStyle::Crlf;
    }
    LineEndingStyle::Lf
}

/// 把文件内容转换为面向模型的视图。
///
/// 纯 CRLF 文件统一展示为 LF，模型据此给出的 old_string 才能匹配上；
/// 其余风格原样保留，避免破坏混用文件的既有结构。
///
/// 参数:
/// - `raw`: 原始文件内容
///
/// 返回:
/// - 模型视图与原始行尾风格
pub(crate) fn to_model_text_view(raw: &str) -> ModelTextView {
    let line_ending_style = detect_line_ending_style(raw);
    if line_ending_style != LineEndingStyle::Crlf {
        return ModelTextView {
            text: raw.to_string(),
            line_ending_style,
        };
    }
    ModelTextView {
        text: raw.replace("\r\n", "\n"),
        line_ending_style,
    }
}

/// 把模型视图的文本还原为磁盘写入格式。
///
/// 参数:
/// - `text`: 模型视图文本
/// - `line_ending_style`: 原文件行尾风格
///
/// 返回:
/// - 可直接写入磁盘的内容
pub(crate) fn materialize_model_text(text: &str, line_ending_style: LineEndingStyle) -> String {
    if line_ending_style != LineEndingStyle::Crlf {
        return text.to_string();
    }
    // 先折叠已有 CRLF，避免二次替换产生 \r\r\n
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯 LF 文件保持原样。
    #[test]
    fn detects_lf_only_content() {
        assert_eq!(detect_line_ending_style("a\nb\n"), LineEndingStyle::Lf);
        let view = to_model_text_view("a\nb\n");
        assert_eq!(view.text, "a\nb\n");
    }

    /// 纯 CRLF 文件对模型展示为 LF。
    #[test]
    fn normalizes_pure_crlf_for_the_model() {
        assert_eq!(
            detect_line_ending_style("a\r\nb\r\n"),
            LineEndingStyle::Crlf
        );
        let view = to_model_text_view("a\r\nb\r\n");
        assert_eq!(view.text, "a\nb\n");
    }

    /// CRLF 与 LF 混用时判定为 Mixed，不做归一化。
    #[test]
    fn keeps_mixed_content_untouched() {
        assert_eq!(detect_line_ending_style("a\r\nb\n"), LineEndingStyle::Mixed);
        let view = to_model_text_view("a\r\nb\n");
        assert_eq!(view.text, "a\r\nb\n");
    }

    /// 孤立的 \r 判定为 Mixed。
    #[test]
    fn treats_lone_carriage_return_as_mixed() {
        assert_eq!(detect_line_ending_style("a\rb"), LineEndingStyle::Mixed);
    }

    /// CRLF 文件写回时还原行尾。
    #[test]
    fn restores_crlf_on_write_back() {
        assert_eq!(
            materialize_model_text("a\nb\n", LineEndingStyle::Crlf),
            "a\r\nb\r\n"
        );
    }

    /// 还原不会把已有 CRLF 变成 \r\r\n。
    #[test]
    fn does_not_double_convert_existing_crlf() {
        assert_eq!(
            materialize_model_text("a\r\nb\n", LineEndingStyle::Crlf),
            "a\r\nb\r\n"
        );
    }

    /// 非 CRLF 风格写回时保持原样。
    #[test]
    fn leaves_lf_content_unchanged_on_write_back() {
        assert_eq!(
            materialize_model_text("a\nb\n", LineEndingStyle::Lf),
            "a\nb\n"
        );
        assert_eq!(
            materialize_model_text("a\r\nb\n", LineEndingStyle::Mixed),
            "a\r\nb\n"
        );
    }
}
