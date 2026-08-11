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
    // #region agent log
    {
        use std::io::Write;
        let looks_like_marker = cell.text.contains("[text ") || cell.text.contains("[image ");
        let prefix_text: String = cell.text.chars().take(80).collect();
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/home/snemc/workspace/sai/.cursor/debug-dcb5f5.log")
        {
            let _ = writeln!(
                f,
                r#"{{"sessionId":"dcb5f5","runId":"pre-fix","hypothesisId":"B","location":"user_echo_cell.rs:render","message":"render user echo","data":{{"looksLikeMarker":{looks_like_marker},"chars":{},"lines":{},"prefix":{}}},"timestamp":{}}}"#,
                cell.text.chars().count(),
                cell.text.lines().count().max(if cell.text.is_empty() { 0 } else { 1 }),
                serde_json::to_string(&prefix_text).unwrap_or_else(|_| "\"\"".into()),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            );
        }
    }
    // #endregion
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
