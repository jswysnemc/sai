use super::ToolCallStreamProgress;

const PROGRESS_BYTE_STEP: usize = 8 * 1024;
const ARGUMENTS_PREVIEW_CHARS: usize = 4096;
const TARGET_KEYS: &[&str] = &[
    "path",
    "name",
    "group_name",
    "tool_name",
    "include",
    "pattern",
    "command",
    "patch",
    "content",
    "replacement",
];

#[derive(Debug, Default)]
pub(crate) struct ToolCallProgressTracker {
    entries: Vec<ToolCallProgressEntry>,
}

/// 未闭合 target 字段内容增长到该字节数时也触发进度，覆盖单行长命令
const TARGET_CONTENT_BYTE_STEP: usize = 48;

#[derive(Debug, Default)]
struct ToolCallProgressEntry {
    emitted: bool,
    last_name: String,
    last_arguments_bytes: usize,
    target_started: bool,
    target_seen: bool,
    /// 未闭合 target 字段已解码内容的换行数，用于命令逐行流式预览
    last_target_newlines: usize,
    /// 未闭合 target 字段已解码内容的字节数
    last_target_content_bytes: usize,
}

impl ToolCallProgressTracker {
    /// 更新工具调用参数接收进度。
    ///
    /// 参数:
    /// - `index`: 工具调用索引
    /// - `name`: 当前已接收到的工具名称
    /// - `arguments`: 当前已接收到的完整参数片段
    ///
    /// 返回:
    /// - 需要向外发送的进度事件，没有新进度时返回空
    pub(crate) fn update(
        &mut self,
        index: usize,
        name: &str,
        arguments: &str,
    ) -> Option<ToolCallStreamProgress> {
        while self.entries.len() <= index {
            self.entries.push(ToolCallProgressEntry::default());
        }
        let entry = &mut self.entries[index];
        let arguments_bytes = arguments.len();
        let name_changed = !name.trim().is_empty() && entry.last_name != name;
        let size_changed =
            arguments_bytes.saturating_sub(entry.last_arguments_bytes) >= PROGRESS_BYTE_STEP;
        let target_started = entry.target_started || has_started_target_field(arguments);
        let target_seen = entry.target_seen || has_complete_target_field(arguments);
        let target_started_changed = target_started && !entry.target_started;
        let target_changed = target_seen && !entry.target_seen;
        // 1. 未闭合 target 字段：按换行或内容增量触发，使 command 块可逐行刷新
        let partial_target = if target_started && !target_seen {
            partial_target_field_content(arguments)
        } else {
            None
        };
        let target_newlines = partial_target
            .as_ref()
            .map(|text| text.matches('\n').count())
            .unwrap_or(0);
        let target_content_bytes = partial_target.as_ref().map(String::len).unwrap_or(0);
        let target_line_progress = target_started
            && !target_seen
            && (target_newlines > entry.last_target_newlines
                || target_content_bytes.saturating_sub(entry.last_target_content_bytes)
                    >= TARGET_CONTENT_BYTE_STEP);
        let first_visible = !entry.emitted && (!name.trim().is_empty() || arguments_bytes > 0);
        if !(first_visible
            || name_changed
            || size_changed
            || target_started_changed
            || target_changed
            || target_line_progress)
        {
            return None;
        }
        entry.emitted = true;
        entry.last_name = name.to_string();
        entry.last_arguments_bytes = arguments_bytes;
        entry.target_started = target_started;
        entry.target_seen = target_seen;
        entry.last_target_newlines = target_newlines;
        entry.last_target_content_bytes = target_content_bytes;
        Some(ToolCallStreamProgress {
            index,
            name: (!name.trim().is_empty()).then(|| name.to_string()),
            arguments_chars: arguments.chars().count(),
            arguments_bytes,
            arguments_preview: arguments.chars().take(ARGUMENTS_PREVIEW_CHARS).collect(),
        })
    }
}

/// 判断参数片段是否已经开始包含 target 字段。
///
/// 参数:
/// - `arguments`: 当前累计参数文本
///
/// 返回:
/// - 是否存在已经开始的 target 字符串字段
fn has_started_target_field(arguments: &str) -> bool {
    TARGET_KEYS
        .iter()
        .any(|key| started_json_string_field(arguments, key))
}

/// 判断参数片段是否已经包含完整 target 字段。
///
/// 参数:
/// - `arguments`: 当前累计参数文本
///
/// 返回:
/// - 是否存在完整 target 字段
fn has_complete_target_field(arguments: &str) -> bool {
    TARGET_KEYS
        .iter()
        .any(|key| complete_json_string_field(arguments, key))
}

/// 判断 JSON 片段中指定字符串字段是否已经开始。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字符串字段是否已经进入值内容
fn started_json_string_field(raw: &str, key: &str) -> bool {
    let pattern = format!("\"{}\"", key);
    let Some(key_index) = raw.find(&pattern) else {
        return false;
    };
    let after_key = &raw[key_index + pattern.len()..];
    let Some(colon_index) = after_key.find(':') else {
        return false;
    };
    after_key[colon_index + 1..].trim_start().starts_with('"')
}

/// 判断 JSON 片段中指定字符串字段是否已经闭合。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字符串字段是否完整
fn complete_json_string_field(raw: &str, key: &str) -> bool {
    let pattern = format!("\"{}\"", key);
    let Some(key_index) = raw.find(&pattern) else {
        return false;
    };
    let after_key = &raw[key_index + pattern.len()..];
    let Some(colon_index) = after_key.find(':') else {
        return false;
    };
    let after_colon = after_key[colon_index + 1..].trim_start();
    let Some(quote_index) = after_colon.find('"') else {
        return false;
    };
    json_string_is_closed(&after_colon[quote_index..])
}

/// 判断 JSON 字符串片段是否已经闭合。
///
/// 参数:
/// - `value`: 以双引号开头的 JSON 字符串片段
///
/// 返回:
/// - 是否找到未转义结束双引号
fn json_string_is_closed(value: &str) -> bool {
    if !value.starts_with('"') {
        return false;
    }
    let mut escaped = false;
    for ch in value.chars().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return true;
        }
    }
    false
}

/// 提取已开始但可能尚未闭合的 target 字段内容。
///
/// 参数:
/// - `arguments`: 当前累计参数文本
///
/// 返回:
/// - 已解码的字段字符串内容（未闭合时返回已收到部分）
fn partial_target_field_content(arguments: &str) -> Option<String> {
    for key in TARGET_KEYS {
        if let Some(content) = partial_json_string_field(arguments, key) {
            return Some(content);
        }
    }
    None
}

/// 从 JSON 片段中提取指定字符串字段的已解码内容。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字段内容；字段未开始时返回空
fn partial_json_string_field(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let quote_index = after_colon.find('"')?;
    decode_json_string_prefix(&after_colon[quote_index..])
}

/// 解码以双引号开头的 JSON 字符串前缀。
///
/// 参数:
/// - `value`: 以双引号开头的字符串片段
///
/// 返回:
/// - 解码后的内容；未闭合时返回已收到内容
fn decode_json_string_prefix(value: &str) -> Option<String> {
    if !value.starts_with('"') {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for ch in value.chars().skip(1) {
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(output);
        }
        output.push(ch);
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_emits_initial_name_and_large_argument_steps() {
        let mut tracker = ToolCallProgressTracker::default();

        let initial = tracker.update(0, "edit_file", "").unwrap();
        assert_eq!(initial.name.as_deref(), Some("edit_file"));
        assert_eq!(initial.arguments_bytes, 0);
        assert_eq!(initial.arguments_preview, "");

        assert!(tracker.update(0, "edit_file", "abc").is_none());

        let large = "x".repeat(PROGRESS_BYTE_STEP);
        let next = tracker.update(0, "edit_file", &large).unwrap();
        assert_eq!(next.arguments_bytes, PROGRESS_BYTE_STEP);
        assert_eq!(next.arguments_preview.len(), ARGUMENTS_PREVIEW_CHARS);
    }

    #[test]
    fn tracker_emits_when_target_field_is_complete() {
        let mut tracker = ToolCallProgressTracker::default();

        let initial = tracker.update(0, "edit_file", "").unwrap();
        assert_eq!(initial.name.as_deref(), Some("edit_file"));

        let started = tracker
            .update(0, "edit_file", r#"{"patch":"*** Begin Pa"#)
            .unwrap();
        assert_eq!(started.arguments_preview, r#"{"patch":"*** Begin Pa"#);

        let target = tracker
            .update(0, "edit_file", r#"{"patch":"*** Begin Patch\n*** End Patch","extra":""#)
            .unwrap();
        assert_eq!(
            target.arguments_preview,
            r#"{"patch":"*** Begin Patch\n*** End Patch","extra":""#
        );
    }

    #[test]
    fn tracker_emits_when_command_field_is_complete() {
        let mut tracker = ToolCallProgressTracker::default();

        let initial = tracker.update(0, "run_command", "").unwrap();
        assert_eq!(initial.name.as_deref(), Some("run_command"));

        let started = tracker
            .update(0, "run_command", r#"{"command":"pwd"#)
            .unwrap();
        assert_eq!(started.arguments_preview, r#"{"command":"pwd"#);

        let target = tracker
            .update(0, "run_command", r#"{"command":"pwd","yield_time_ms":"#)
            .unwrap();
        assert_eq!(
            target.arguments_preview,
            r#"{"command":"pwd","yield_time_ms":"#
        );
    }

    #[test]
    fn tracker_emits_when_patch_field_is_complete() {
        let mut tracker = ToolCallProgressTracker::default();

        let initial = tracker.update(0, "edit_file", "").unwrap();
        assert_eq!(initial.name.as_deref(), Some("edit_file"));

        let started = tracker
            .update(0, "edit_file", r#"{"patch":"*** Begin Patch"#)
            .unwrap();
        assert!(started.arguments_preview.contains("*** Begin Patch"));

        let target = tracker
            .update(
                0,
                "edit_file",
                r#"{"patch":"*** Begin Patch\n*** End Patch","path":"#,
            )
            .unwrap();
        assert!(target.arguments_preview.contains("*** Begin Patch"));
    }

    #[test]
    fn tracker_emits_when_target_field_starts() {
        let mut tracker = ToolCallProgressTracker::default();

        let initial = tracker.update(0, "run_command", "").unwrap();
        assert_eq!(initial.name.as_deref(), Some("run_command"));

        assert!(tracker.update(0, "run_command", r#"{"com"#).is_none());

        let target = tracker
            .update(0, "run_command", r#"{"command":"echo"#)
            .unwrap();
        assert_eq!(target.arguments_preview, r#"{"command":"echo"#);
    }

    #[test]
    fn tracker_emits_on_command_newline_progress() {
        let mut tracker = ToolCallProgressTracker::default();

        let _ = tracker.update(0, "run_command", "").unwrap();
        let started = tracker
            .update(0, "run_command", r#"{"command":"line1"#)
            .unwrap();
        assert_eq!(started.arguments_preview, r#"{"command":"line1"#);

        // 1. 同一行继续增长且未达步进阈值时不重复发送
        assert!(tracker
            .update(0, "run_command", r#"{"command":"line1 more"#)
            .is_none());

        // 2. 出现新换行时发送进度，便于命令块逐行刷新
        let line2 = tracker
            .update(0, "run_command", r#"{"command":"line1\nline2"#)
            .unwrap();
        assert!(line2.arguments_preview.contains(r#"line1\nline2"#));

        let line3 = tracker
            .update(0, "run_command", r#"{"command":"line1\nline2\nline3"#)
            .unwrap();
        assert!(line3.arguments_preview.contains(r#"line3"#));
    }

    #[test]
    fn tracker_emits_on_long_single_line_command_growth() {
        let mut tracker = ToolCallProgressTracker::default();

        let _ = tracker.update(0, "run_command", "").unwrap();
        let started = tracker
            .update(0, "run_command", r#"{"command":"echo "#)
            .unwrap();
        assert!(started.arguments_preview.contains("echo"));

        let long = format!(
            r#"{{"command":"echo {}"#,
            "x".repeat(TARGET_CONTENT_BYTE_STEP)
        );
        let grown = tracker.update(0, "run_command", &long).unwrap();
        assert!(grown.arguments_preview.contains("echo"));
    }
}
