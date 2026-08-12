/// 系统提示的语义类别，决定行首符号与着色。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MetaKind {
    /// 控制命令回执、状态说明等中性提示
    Notice,
    /// 轮次失败、中断等需要用户注意的错误
    Failure,
    /// 轮次结束的上下文总览；渲染层在其下追加 turn 分割线
    Summary,
}

/// REPL 系统提示、控制命令与错误的 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MetaCell {
    pub(crate) text: String,
    pub(crate) kind: MetaKind,
}

/// 渲染系统提示或控制命令消息。
///
/// 按类别给出行首符号：失败用红色叉号、首行保持常规亮度；
/// 提示用弱化的 `›`。与工具行的 `•`、思考的 `◦`、无符号正文缩进区分开。
///
/// 已带 ANSI 样式的文本（如会话摘要）自带完整排版，原样输出。
///
/// 参数:
/// - `cell`: 元信息源数据
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &MetaCell) -> String {
    // 1. 自带样式的内容拥有自己的行首符号与配色，不再包一层
    if cell.text.contains('\x1b') {
        return cell.text.clone();
    }
    let mut lines = cell.text.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    // 2. 首行带类别符号，续行缩进对齐并统一弱化
    let mut output = match cell.kind {
        // 整行失败色：终端若缺 ✗ 字形，至少标题仍是醒目的红色引导
        MetaKind::Failure => format!("\x1b[31m✗ {first}\x1b[0m"),
        // 总览正常情况下自带样式走上方 verbatim 分支；纯文本兜底按中性提示排版
        MetaKind::Notice | MetaKind::Summary => format!("\x1b[2m› {first}\x1b[0m"),
    };
    for line in lines {
        let styled = match cell.kind {
            MetaKind::Failure => format!("\n  \x1b[31m\x1b[2m{line}\x1b[0m"),
            MetaKind::Notice | MetaKind::Summary => format!("\n  \x1b[2m{line}\x1b[0m"),
        };
        output.push_str(&styled);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 失败提示首行带红色叉号且保持常规亮度，续行弱化缩进。
    #[test]
    fn failures_carry_a_red_mark_and_keep_the_first_line_bright() {
        let rendered = render(&MetaCell {
            text: "本轮失败：流式响应为空\n对话内容保持完整，可重新发送。".to_string(),
            kind: MetaKind::Failure,
        });

        assert!(
            rendered.starts_with("\x1b[31m✗ 本轮失败"),
            "{rendered:?}"
        );
        assert!(rendered.contains("\n  \x1b[31m\x1b[2m对话内容保持完整"));
        // 首行不整体压暗，否则与思考正文混在一起
        assert!(!rendered.starts_with("\x1b[2m"));
    }

    /// 中性提示带弱化引导符，与正文的引导点区分。
    #[test]
    fn notices_carry_a_dim_guide_glyph() {
        let rendered = render(&MetaCell {
            text: "已切换模型".to_string(),
            kind: MetaKind::Notice,
        });

        assert_eq!(rendered, "\x1b[2m› 已切换模型\x1b[0m");
    }

    /// 自带 ANSI 样式的内容原样输出，不重复包装。
    #[test]
    fn self_styled_content_is_rendered_verbatim() {
        let text = "\x1b[2m•\x1b[0m Context: 26k".to_string();
        let rendered = render(&MetaCell {
            text: text.clone(),
            kind: MetaKind::Notice,
        });

        assert_eq!(rendered, text);
    }
}
