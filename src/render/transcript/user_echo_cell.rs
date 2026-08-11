use super::cell::TranscriptMode;
use crate::i18n::text as t;
use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES,
};

/// 用户输入回显的 source-backed 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UserEchoCell {
    pub(crate) mode: TranscriptMode,
    pub(crate) text: String,
    /// 是否展开完整正文；仅 `fold == true` 时有意义。
    pub(crate) expanded: bool,
    /// 粘贴长文本才做思考式折叠；普通键入长消息保持全文。
    pub(crate) fold: bool,
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
/// 仅粘贴长文本（`cell.fold`）按显示行首尾折叠（前 2 后 4），中间省略并提示 Ctrl+O；
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
    let body = cell.text.trim_end();
    if body.is_empty() {
        return format!("\n{prefix}");
    }
    // 续行缩进两列；按净宽折行后再按需折叠
    let wrap = terminal_wrap_width().saturating_sub(2).max(8);
    let wrapped = wrap_display_lines(body, wrap);
    let (visible, omitted) = if cell.fold {
        fold_display_lines(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, cell.expanded)
    } else {
        (wrapped.clone(), 0)
    };
    let mut lines = Vec::with_capacity(visible.len());
    let mut content_index = 0usize;
    for line in visible {
        if line == "__OMITTED__" {
            lines.push(format!(
                "  \x1b[2m… +{omitted} {} (Ctrl+O {})\x1b[0m",
                t("lines", "行"),
                t("to expand", "展开")
            ));
            continue;
        }
        if content_index == 0 {
            lines.push(format!("{prefix}{line}"));
        } else {
            lines.push(format!("  {line}"));
        }
        content_index += 1;
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
}
