use crate::llm::ChatMessage;

/// 将旧工具报告投影为 display-only provider 文本。
///
/// 参数:
/// - `reports`: 旧工具报告列表
///
/// 返回:
/// - 普通 assistant 文本消息列表，不包含 provider tool_call_id
pub(in crate::state) fn project_legacy_tool_report_messages(
    reports: &[String],
) -> Vec<ChatMessage> {
    reports
        .iter()
        .map(|report| ChatMessage::plain("assistant", report.clone()))
        .collect()
}

/// 格式化旧工具报告。
///
/// 参数:
/// - `reports`: 旧工具报告列表
/// - `max_chars`: 单条报告最大字符数
///
/// 返回:
/// - 裁剪后的 display-only 工具报告文本
pub(in crate::state) fn format_legacy_tool_reports(reports: &[String], max_chars: usize) -> String {
    reports
        .iter()
        .map(|report| truncate_chars(report.trim(), max_chars))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 按字符数截断文本。
///
/// 参数:
/// - `value`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本
fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut iter = value.chars();
    let truncated = iter.by_ref().take(max_chars).collect::<String>();
    if iter.next().is_some() {
        format!("{truncated}\n[truncated]")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_legacy_reports_as_plain_assistant_messages() {
        let messages = project_legacy_tool_report_messages(&["report".to_string()]);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        assert!(matches!(
            messages[0].content.as_ref(),
            Some(crate::llm::ChatContent::Text(text)) if text == "report"
        ));
        assert!(messages[0].tool_calls.is_none());
        assert!(messages[0].tool_call_id.is_none());
    }

    #[test]
    fn formats_legacy_reports_with_stable_truncation() {
        let text = format_legacy_tool_reports(&["abcdef".to_string(), "xy".to_string()], 3);

        assert_eq!(text, "abc\n[truncated]\n\nxy");
    }
}
