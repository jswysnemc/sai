use super::cell::TranscriptMode;
use super::AnsiLine;
use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FoldedDisplayLine,
    FOLD_HEAD_LINES, FOLD_TAIL_LINES,
};
use crate::render::input_atom::{render_input_atoms, InputAtom, InputAtomKind, InputEcho};

/// 用户输入回显的 source-backed 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UserEchoCell {
    pub(crate) mode: TranscriptMode,
    pub(crate) text: String,
    /// 是否展开完整正文；仅 `fold == true` 时有意义。
    pub(crate) expanded: bool,
    /// 粘贴长文本才做思考式折叠；普通键入长消息保持全文。
    pub(crate) fold: bool,
    /// 真实附件来源及其在完整正文中的范围
    pub(crate) atoms: Vec<InputAtom>,
}

impl UserEchoCell {
    /// 创建不折叠的用户回显单元（普通键入 / 历史恢复）。
    ///
    /// 参数:
    /// - `mode`: 提交时的 REPL 模式
    /// - `text`: 原始输入文本
    ///
    /// 返回:
    /// - 用户回显单元
    pub(crate) fn new(mode: TranscriptMode, text: String) -> Self {
        Self::with_fold(mode, text, false)
    }

    /// 创建用户回显单元。
    ///
    /// 参数:
    /// - `mode`: 提交时的 REPL 模式
    /// - `text`: 回显正文（粘贴块应已展开）
    /// - `fold`: 是否按思考块语义折叠
    ///
    /// 返回:
    /// - 用户回显单元
    pub(crate) fn with_fold(mode: TranscriptMode, text: String, fold: bool) -> Self {
        Self {
            mode,
            text,
            expanded: false,
            fold,
            atoms: Vec::new(),
        }
    }

    /// 创建携带真实原子块元数据的用户回显。
    ///
    /// 参数: `mode` 为提交模式，`echo` 为完整正文与已登记原子块
    /// 返回: 默认显示附件标签的回显单元
    pub(crate) fn with_atoms(mode: TranscriptMode, echo: InputEcho) -> Self {
        Self {
            mode,
            text: echo.text,
            expanded: false,
            fold: false,
            atoms: echo.atoms,
        }
    }

    /// 切换展开/折叠状态。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
}

/// 渲染用户提交后的输入回显。
///
/// 已登记附件显示标签，长文本通过 Ctrl+O 展开；旧记录沿用首尾预览。
/// 普通键入消息无论多长都全文展示。
///
/// 参数:
/// - `cell`: 用户输入回显源
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &UserEchoCell) -> String {
    let prefix = match cell.mode {
        TranscriptMode::Yolo => "\x1b[38;5;208m●\x1b[0m ",
        TranscriptMode::Plan => "\x1b[36m●\x1b[0m ",
        TranscriptMode::Automatic => "\x1b[38;5;39m●\x1b[0m ",
    };
    let expanded = cell.expanded || crate::render::render_expand::expand_override();
    let styled = render_input_atoms(&cell.text, &cell.atoms, expanded);
    let body = styled.trim_end();
    if body.is_empty() {
        return format!("\n{prefix}");
    }
    // 续行缩进两列；按净宽折行后再按需折叠
    let wrap = terminal_wrap_width().saturating_sub(2).max(8);
    let wrapped: Vec<String> = AnsiLine::wrap_block(body, wrap)
        .into_iter()
        .map(|line| line.as_str().to_string())
        .collect();
    let visible = if cell.fold {
        fold_display_lines(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded)
    } else {
        wrapped
            .iter()
            .cloned()
            .map(FoldedDisplayLine::Line)
            .collect()
    };
    let mut lines = Vec::with_capacity(visible.len());
    let mut content_index = 0usize;
    for line in visible {
        let line = match line {
            FoldedDisplayLine::Omitted { omitted, .. } => {
                lines.push(crate::render::omitted_line::render_omitted_line(
                    omitted, true,
                ));
                continue;
            }
            FoldedDisplayLine::Line(line) => line,
        };
        if content_index == 0 {
            lines.push(format!("{prefix}{line}"));
        } else {
            lines.push(format!("  {line}"));
        }
        content_index += 1;
    }
    if !expanded {
        let hidden = cell
            .atoms
            .iter()
            .filter(|atom| atom.kind == InputAtomKind::Text)
            .filter_map(|atom| cell.text.get(atom.range.clone()))
            .map(|text| wrap_display_lines(text, wrap).len())
            .sum();
        if hidden > 0 {
            lines.push(crate::render::omitted_line::render_omitted_line(
                hidden, true,
            ));
        }
    }
    // 轮次前空一行，和上一轮总览/响应隔开
    format!("\n{}", lines.join("\n"))
}

/// 判断用户回显是否应按粘贴折叠语义处理（含 Ctrl+O）。
///
/// 参数:
/// - `cell`: 用户回显单元
///
/// 返回:
/// - 粘贴长文本且默认宽度下会省略中间行时为 true
pub(crate) fn would_fold(cell: &UserEchoCell) -> bool {
    if cell
        .atoms
        .iter()
        .any(|atom| atom.kind == InputAtomKind::Text)
    {
        return true;
    }
    if !cell.fold {
        return false;
    }
    let body = cell.text.trim_end();
    if body.is_empty() {
        return false;
    }
    let wrap = terminal_wrap_width().saturating_sub(2).max(8);
    let wrapped = wrap_display_lines(body, wrap);
    wrapped.len() > FOLD_HEAD_LINES.saturating_add(FOLD_TAIL_LINES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_echo_is_not_folded() {
        let rendered = render(&UserEchoCell::new(
            TranscriptMode::Yolo,
            "hello".to_string(),
        ));
        assert!(rendered.contains("hello"));
        assert!(!rendered.contains("Ctrl+O"));
        assert!(!rendered.contains('…'));
    }

    #[test]
    fn typed_long_echo_is_not_folded() {
        let text = (0..20)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render(&UserEchoCell::new(TranscriptMode::Yolo, text));
        assert!(rendered.contains("line-10"));
        assert!(!rendered.contains("Ctrl+O"));
    }

    #[test]
    fn pasted_long_echo_folds_with_expand_hint() {
        let text = (0..20)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let collapsed = render(&UserEchoCell::with_fold(
            TranscriptMode::Yolo,
            text.clone(),
            true,
        ));
        assert!(collapsed.contains("line-0"));
        assert!(collapsed.contains("line-19"));
        assert!(collapsed.contains("Ctrl+O"));
        assert!(!collapsed.contains("line-10"));

        let mut cell = UserEchoCell::with_fold(TranscriptMode::Yolo, text, true);
        cell.expanded = true;
        let expanded = render(&cell);
        assert!(expanded.contains("line-10"));
        assert!(!expanded.contains("Ctrl+O"));
    }

    /// 没有附件来源的方括号文本不能获得附件样式。
    #[test]
    fn literal_image_label_remains_plain_text() {
        let rendered = render(&UserEchoCell::new(
            TranscriptMode::Yolo,
            "[image 1 800x600] [ordinary]".to_string(),
        ));
        assert!(!rendered.contains("\x1b[48;5;89m"));
        assert!(rendered.contains("[image 1 800x600] [ordinary]"));
    }
}
