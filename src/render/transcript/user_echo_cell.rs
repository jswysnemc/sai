use super::cell::TranscriptMode;

/// 用户输入回显的 source-backed 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UserEchoCell {
    pub(crate) mode: TranscriptMode,
    pub(crate) text: String,
}

/// 渲染用户提交后的输入回显。
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
    // 轮次前空一行，和上一轮响应轻微隔开
    let body = cell
        .text
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                format!("{prefix}{line}")
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n{body}")
}
