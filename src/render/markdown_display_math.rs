/// 一行 Markdown 与 `$$` 块级公式的关系。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DisplayMathLine {
    /// 整行为 `$$公式$$`
    Single(String),
    /// 开启块级公式；`$$` 后同一行的公式内容
    Open(Option<String>),
    /// 结束块级公式；`$$` 前同一行的公式内容
    Close(Option<String>),
    /// 块内普通公式行，或块外的普通文本
    Other,
}

/// 【终端】【块级公式】识别 `$$` 在行首或行尾的块级公式写法。
///
/// 模型常把定界符与首尾公式写在同一行，例如 `$$\begin{aligned}` 与
/// `\end{aligned}$$`；只认独占一行的 `$$` 时，这类行会落到行内解析，
/// 被拆成空公式与残余源码。
///
/// 参数:
/// - `line`: 当前行
/// - `in_block`: 是否已在块级公式中
///
/// 返回:
/// - 本行的块级公式角色
pub(crate) fn classify_display_math_line(line: &str, in_block: bool) -> DisplayMathLine {
    let trimmed = line.trim();
    if in_block {
        // 1. 块内：以 `$$` 结尾即闭合，前面的内容仍属于公式
        return match trimmed.strip_suffix("$$") {
            Some(rest) => DisplayMathLine::Close(non_empty(rest)),
            None => DisplayMathLine::Other,
        };
    }
    let Some(rest) = trimmed.strip_prefix("$$") else {
        return DisplayMathLine::Other;
    };
    // 2. 块外：`$$…$$` 独占一行是单行块级公式；中间再出现 `$$` 说明是多个行内公式
    if let Some(inner) = rest.strip_suffix("$$") {
        if inner.contains("$$") {
            return DisplayMathLine::Other;
        }
        return match non_empty(inner) {
            Some(formula) => DisplayMathLine::Single(formula),
            None => DisplayMathLine::Other,
        };
    }
    // 3. 行首 `$$` 后面还有闭合定界符时交给行内解析，例如 `$$x$$。`
    if rest.contains("$$") {
        return DisplayMathLine::Other;
    }
    DisplayMathLine::Open(non_empty(rest))
}

/// 去除首尾空白后返回非空内容。
fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证独占一行的 `$$…$$` 按块级公式处理。
    #[test]
    fn whole_line_formula_is_display_math() {
        assert_eq!(
            classify_display_math_line("  $$ e^{i\\pi} + 1 = 0 $$ ", false),
            DisplayMathLine::Single("e^{i\\pi} + 1 = 0".to_string())
        );
    }

    /// 验证定界符与公式同行的开合写法。
    #[test]
    fn delimiters_sharing_a_line_open_and_close_blocks() {
        assert_eq!(
            classify_display_math_line("$$\\begin{aligned}", false),
            DisplayMathLine::Open(Some("\\begin{aligned}".to_string()))
        );
        assert_eq!(
            classify_display_math_line("$$", false),
            DisplayMathLine::Open(None)
        );
        assert_eq!(
            classify_display_math_line("\\end{aligned}$$", true),
            DisplayMathLine::Close(Some("\\end{aligned}".to_string()))
        );
        assert_eq!(
            classify_display_math_line("$$", true),
            DisplayMathLine::Close(None)
        );
        assert_eq!(
            classify_display_math_line("\\nabla \\cdot E &= 0 \\\\", true),
            DisplayMathLine::Other
        );
    }

    /// 验证行内用法与空公式不被当成块级公式。
    #[test]
    fn inline_usages_stay_inline() {
        for line in [
            "质能公式 $$E=mc^2$$ 很有名",
            "$$a$$ 与 $$b$$",
            "$$x$$。",
            "$$$$",
            "普通文本",
        ] {
            assert_eq!(
                classify_display_math_line(line, false),
                DisplayMathLine::Other,
                "{line}"
            );
        }
    }
}
