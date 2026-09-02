use super::cell::TranscriptMode;
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
            lines.push(crate::render::omitted_line::render_omitted_line(
                omitted, true,
            ));
            continue;
        }
        // 图片占位块（[image N WxH]）用洋红色标注，与普通文本区分；
        // 长文本占位块由折叠语义处理，保持原有弱化色
        let styled = if is_image_placeholder(&line) {
            style_image_placeholder(&line)
        } else {
            line.clone()
        };
        if content_index == 0 {
            lines.push(format!("{prefix}{styled}"));
        } else {
            lines.push(format!("  {styled}"));
        }
        content_index += 1;
    }
    // 轮次前空一行，和上一轮总览/响应隔开
    format!("\n{}", lines.join("\n"))
}

/// 判断回显行是否为图片占位块。
///
/// 图片粘贴在输入框中被原子化为 `[image N WxH]` 标记，回显时需要
/// 与普通文本区分，让用户确认附件仍然存在。
///
/// 参数:
/// - `line`: 回显行文本
///
/// 返回:
/// - 是图片占位块时返回 true
fn is_image_placeholder(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with("[image ") || !trimmed.ends_with(']') {
        return false;
    }
    let inner = &trimmed["[image ".len()..trimmed.len() - 1];
    let mut parts = inner.split_whitespace();
    let index = parts.next().unwrap_or_default();
    let size = parts.next().unwrap_or_default();
    let tail = parts.next();
    // 结构固定为两个词：序号 + WxH 尺寸
    !index.is_empty()
        && !size.is_empty()
        && tail.is_none()
        && size.contains('x')
        && index.chars().all(|ch| ch.is_ascii_digit())
}

/// 渲染图片占位块：洋红色调的附件标签样式。
///
/// 参数:
/// - `line`: 图片占位块行
///
/// 返回:
/// - 着色后的占位块文本
fn style_image_placeholder(line: &str) -> String {
    format!("\x1b[35m{line}\x1b[0m")
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

    /// 图片占位块回显时被识别并着洋红色，普通文本不受影响。
    #[test]
    fn image_placeholder_is_tinted_magenta() {
        assert!(is_image_placeholder("[image 1 800x600]"));
        assert!(is_image_placeholder("  [image 12 64x64]  "));
        assert!(!is_image_placeholder("[text 1 2000 chars]"));
        assert!(!is_image_placeholder("normal message"));
        assert!(!is_image_placeholder("[image 1 800x600 extra]"));

        let styled = style_image_placeholder("[image 1 800x600]");
        assert!(styled.contains("\x1b[35m"));
        assert!(styled.contains("[image 1 800x600]"));
    }
}
