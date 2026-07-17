use crate::render::tool_event_line;

/// 生成工具状态文本。
///
/// 参数:
/// - `name`: 工具展示标签
/// - `status`: 工具状态
///
/// 返回:
/// - 可直接写入终端的单行状态文本
pub(crate) fn tool_call_status_text(name: &str, status: &str) -> String {
    tool_event_line::tool_call_status_text(name, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_uses_compact_state() {
        let output = tool_call_status_text("Write main.rs", "arg");

        assert!(output.contains("Write main.rs"));
        assert!(output.contains("\x1b[36m...\x1b[0m"));
        assert!(!output.contains("receiving"));
        assert!(!output.contains("arg"));
        assert!(!output.contains("tool:"));
    }
}
