const DEFAULT_TOOL_CONTEXT_MAX_CHARS: usize = 24_000;
const COMMAND_TOOL_CONTEXT_MAX_CHARS: usize = 16_000;
const SEARCH_TOOL_CONTEXT_MAX_CHARS: usize = 18_000;

/// 返回用于模型上下文的工具输出。
///
/// 参数:
/// - `tool_name`: 工具名称
/// - `output`: 原始工具输出
///
/// 返回:
/// - 适合继续放入模型上下文的工具输出
pub(crate) fn tool_output_for_context(tool_name: &str, output: &str) -> String {
    let limit = tool_context_char_limit(tool_name);
    let total = output.chars().count();
    if total <= limit {
        return output.to_string();
    }
    let clipped = output.chars().take(limit).collect::<String>();
    format!(
        "{clipped}\n\n[tool output clipped from {total} chars to {limit} chars for model context; rerun the tool with narrower query, pagination, or max_chars if more detail is needed]"
    )
}

/// 返回工具上下文字符上限。
///
/// 参数:
/// - `tool_name`: 工具名称
///
/// 返回:
/// - 字符上限
fn tool_context_char_limit(tool_name: &str) -> usize {
    match tool_name {
        "run_command" | "background_command" => COMMAND_TOOL_CONTEXT_MAX_CHARS,
        "web_search"
        | "web_fetch"
        | "search_knowledge_base"
        | "search_knowledge_base_by_name"
        | "read_knowledge_base_file"
        | "grep"
        | "search_text" // 历史别名，仍按搜索类截断
        | "glob"
        | "find_files" // 历史别名，仍按搜索类截断
        | "read_file" => SEARCH_TOOL_CONTEXT_MAX_CHARS,
        _ => DEFAULT_TOOL_CONTEXT_MAX_CHARS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_output_for_context_clips_large_outputs() {
        let output = "x".repeat(COMMAND_TOOL_CONTEXT_MAX_CHARS + 1);
        let clipped = tool_output_for_context("run_command", &output);
        let retained_output = "x".repeat(COMMAND_TOOL_CONTEXT_MAX_CHARS);

        assert!(clipped.contains("tool output clipped"));
        assert!(clipped.starts_with(&retained_output));
        assert!(!clipped.starts_with(&output));
    }

    #[test]
    fn tool_output_for_context_uses_search_limit_for_grep() {
        let output = "x".repeat(SEARCH_TOOL_CONTEXT_MAX_CHARS + 1);
        let clipped = tool_output_for_context("grep", &output);
        let retained_output = "x".repeat(SEARCH_TOOL_CONTEXT_MAX_CHARS);

        assert!(clipped.contains("tool output clipped"));
        assert!(clipped.starts_with(&retained_output));
        assert!(!clipped.starts_with(&output));
    }
}
