use super::ToolCallStreamProgress;

const PROGRESS_BYTE_STEP: usize = 8 * 1024;
const ARGUMENTS_PREVIEW_CHARS: usize = 4096;
/// 编辑/命令正文字段预览上限：需覆盖流式 +N -M 跳动，不能卡在 4K
const BODY_ARGUMENTS_PREVIEW_CHARS: usize = 64 * 1024;
/// 身份字段：闭合后仍应继续跟踪正文字段进度（write_file 的 path 先到）
const IDENTITY_KEYS: &[&str] = &[
    "path",
    "name",
    "group_name",
    "tool_name",
    "include",
    "pattern",
];
/// 正文字段：未闭合时按换行/字节步进发射进度，驱动 Writing +N -M 跳动
const BODY_KEYS: &[&str] = &[
    "command",
    "patch",
    "content",
    "replacement",
    "old_string",
    "new_string",
];

#[derive(Debug, Default)]
pub(crate) struct ToolCallProgressTracker {
    entries: Vec<ToolCallProgressEntry>,
}

/// 未闭合正文内容增长到该字节数时也触发进度，覆盖单行长命令
const TARGET_CONTENT_BYTE_STEP: usize = 48;

#[derive(Debug, Default)]
struct ToolCallProgressEntry {
    emitted: bool,
    last_name: String,
    last_arguments_bytes: usize,
    identity_started: bool,
    identity_seen: bool,
    body_started: bool,
    /// 上一拍是否仍有未闭合正文字段（用于字段闭合时补发一次）
    body_open: bool,
    /// 未闭合正文字段已解码内容的换行数
    last_body_newlines: usize,
    /// 未闭合正文字段已解码内容的字节数
    last_body_content_bytes: usize,
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
        let identity_started =
            entry.identity_started || has_started_any_field(arguments, IDENTITY_KEYS);
        let identity_seen = entry.identity_seen || has_complete_any_field(arguments, IDENTITY_KEYS);
        let body_started = entry.body_started || has_started_any_field(arguments, BODY_KEYS);
        let identity_started_changed = identity_started && !entry.identity_started;
        let identity_changed = identity_seen && !entry.identity_seen;
        let body_started_changed = body_started && !entry.body_started;
        // 只跟踪仍未闭合的正文字段；path 闭合不得关掉 content/new_string 的换行步进
        let partial_body = partial_open_body_field_content(arguments);
        let body_open = partial_body.is_some();
        let body_closed_changed = entry.body_open && !body_open;
        let body_newlines = partial_body
            .as_ref()
            .map(|text| text.matches('\n').count())
            .unwrap_or(0);
        let body_content_bytes = partial_body.as_ref().map(String::len).unwrap_or(0);
        let body_line_progress = body_open
            && (body_newlines > entry.last_body_newlines
                || body_content_bytes.saturating_sub(entry.last_body_content_bytes)
                    >= TARGET_CONTENT_BYTE_STEP
                // 首个非空正文也要发，否则 Writing 会卡在无 +N 直到换行/48B
                || (body_content_bytes > 0 && entry.last_body_content_bytes == 0));
        let first_visible = !entry.emitted && (!name.trim().is_empty() || arguments_bytes > 0);
        if !(first_visible
            || name_changed
            || size_changed
            || identity_started_changed
            || identity_changed
            || body_started_changed
            || body_line_progress
            || body_closed_changed)
        {
            return None;
        }
        entry.emitted = true;
        entry.last_name = name.to_string();
        entry.last_arguments_bytes = arguments_bytes;
        entry.identity_started = identity_started;
        entry.identity_seen = identity_seen;
        entry.body_started = body_started;
        entry.body_open = body_open;
        entry.last_body_newlines = body_newlines;
        entry.last_body_content_bytes = body_content_bytes;
        let preview_limit = if body_started {
            BODY_ARGUMENTS_PREVIEW_CHARS
        } else {
            ARGUMENTS_PREVIEW_CHARS
        };
        Some(ToolCallStreamProgress {
            index,
            name: (!name.trim().is_empty()).then(|| name.to_string()),
            arguments_chars: arguments.chars().count(),
            arguments_bytes,
            arguments_preview: arguments.chars().take(preview_limit).collect(),
        })
    }
}

/// 判断参数片段是否已开始包含给定字段之一。
fn has_started_any_field(arguments: &str, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| started_json_string_field(arguments, key))
}

/// 判断参数片段是否已包含完整的给定字段之一。
fn has_complete_any_field(arguments: &str, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| complete_json_string_field(arguments, key))
}

/// 提取仍未闭合的正文字段内容（优先 content/new_string 等，忽略已闭合的 path）。
fn partial_open_body_field_content(arguments: &str) -> Option<String> {
    for key in BODY_KEYS {
        if started_json_string_field(arguments, key)
            && !complete_json_string_field(arguments, key)
        {
            return partial_json_string_field(arguments, key);
        }
    }
    None
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
            .update(
                0,
                "edit_file",
                r#"{"patch":"*** Begin Patch\n*** End Patch","extra":""#,
            )
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

    /// path 闭合后 content 每增一行仍应 emit，供 Writing +N -M 跳动。
    #[test]
    fn tracker_emits_write_content_lines_after_path_closes() {
        let mut tracker = ToolCallProgressTracker::default();
        let _ = tracker.update(0, "write_file", "").unwrap();
        let path_done = tracker
            .update(0, "write_file", r#"{"path":"notes.md","content":""#)
            .unwrap();
        assert!(path_done.arguments_preview.contains("notes.md"));

        let line1 = tracker
            .update(
                0,
                "write_file",
                r#"{"path":"notes.md","content":"alpha"#,
            )
            .unwrap();
        assert!(line1.arguments_preview.contains("alpha"));

        // 同行小增长不重复发
        assert!(tracker
            .update(
                0,
                "write_file",
                r#"{"path":"notes.md","content":"alpha!"#,
            )
            .is_none());

        let line2 = tracker
            .update(
                0,
                "write_file",
                r#"{"path":"notes.md","content":"alpha\nbeta"#,
            )
            .unwrap();
        assert!(line2.arguments_preview.contains(r#"alpha\nbeta"#));

        let line3 = tracker
            .update(
                0,
                "write_file",
                r#"{"path":"notes.md","content":"alpha\nbeta\ngamma"#,
            )
            .unwrap();
        assert!(line3.arguments_preview.contains("gamma"));
        // preview 需保留已到 content，供 UI streamed_diff_counts 算出 +3
        assert_eq!(
            line3.arguments_preview.matches(r"\n").count(),
            2,
            "open content must keep newlines in preview: {}",
            line3.arguments_preview
        );
    }

    /// str_replace：old_string 闭合后 new_string 换行仍继续 emit。
    #[test]
    fn tracker_emits_new_string_after_old_string_closes() {
        let mut tracker = ToolCallProgressTracker::default();
        let _ = tracker.update(0, "str_replace", "").unwrap();
        let _ = tracker
            .update(
                0,
                "str_replace",
                r#"{"path":"a.rs","old_string":"old\ntext","new_string":""#,
            )
            .unwrap();
        let grown = tracker
            .update(
                0,
                "str_replace",
                r#"{"path":"a.rs","old_string":"old\ntext","new_string":"new\nline"#,
            )
            .unwrap();
        assert!(grown.arguments_preview.contains(r#"new\nline"#));
    }
}
