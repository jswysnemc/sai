/// 命令调用参数流式预览状态。
pub(crate) struct StreamingCommandBlock {
    last_command: String,
    rendered_rows: usize,
}

/// 可流式替换的命令块预览。
pub(crate) struct StreamingCommandPreview {
    pub(crate) command: String,
    pub(crate) clear_rows: usize,
}

impl StreamingCommandBlock {
    /// 创建命令调用参数流式预览状态。
    ///
    /// 返回:
    /// - 新的命令块预览状态
    pub(crate) fn new() -> Self {
        Self {
            last_command: String::new(),
            rendered_rows: 0,
        }
    }

    /// 根据工具参数预览提取可提前渲染的命令。
    ///
    /// 参数:
    /// - `tool_name`: 工具名称
    /// - `arguments_preview`: 当前已收到的参数预览
    ///
    /// 返回:
    /// - 提取成功且命令内容变化时返回命令文本和需要清理的旧块行数
    pub(crate) fn command_from_progress(
        &mut self,
        tool_name: &str,
        arguments_preview: &str,
    ) -> Option<StreamingCommandPreview> {
        if tool_name != "run_command" {
            return None;
        }
        let command = json_string_field(arguments_preview, "command")?;
        if command.trim().is_empty() || command == self.last_command {
            return None;
        }
        let clear_rows = self.rendered_rows;
        self.last_command = command.clone();
        Some(StreamingCommandPreview {
            command,
            clear_rows,
        })
    }

    /// 记录最近一次命令预览占用的视觉行数。
    ///
    /// 参数:
    /// - `rows`: 命令块占用的视觉行数
    ///
    /// 返回:
    /// - 无
    pub(crate) fn mark_rendered_rows(&mut self, rows: usize) {
        self.rendered_rows = rows;
    }

    /// 取出已经提前渲染的命令块视觉行数并重置状态。
    ///
    /// 返回:
    /// - 需要清除的旧命令块视觉行数
    pub(crate) fn take_rendered_rows(&mut self) -> usize {
        self.last_command.clear();
        std::mem::take(&mut self.rendered_rows)
    }

    /// 返回当前预览块占用的视觉行数。
    ///
    /// 返回:
    /// - 当前已渲染预览块的视觉行数
    #[cfg(test)]
    pub(crate) fn rendered_rows(&self) -> usize {
        self.rendered_rows
    }
}

/// 从完整或局部 JSON 文本中提取字符串字段。
///
/// 参数:
/// - `raw`: 原始 JSON 或 JSON 片段
/// - `key`: 字段名
///
/// 返回:
/// - 字符串字段内容，字段已经开始但未闭合时返回已收到内容
fn json_string_field(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let quote_index = after_colon.find('"')?;
    parse_json_string(&after_colon[quote_index..])
}

/// 解析以双引号开头的 JSON 字符串片段。
///
/// 参数:
/// - `value`: 以双引号开头的字符串片段
///
/// 返回:
/// - 解析后的字符串，未闭合时返回已收到内容
fn parse_json_string(value: &str) -> Option<String> {
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
    fn extracts_closed_command_from_partial_json() {
        let mut block = StreamingCommandBlock::new();

        let preview = block
            .command_from_progress("run_command", r#"{"command":"printf \"ok\"\n","yield"#)
            .unwrap();

        assert_eq!(preview.command, "printf \"ok\"\n");
        assert_eq!(preview.clear_rows, 0);
        block.mark_rendered_rows(4);
        let preview = block
            .command_from_progress("run_command", r#"{"command":"date"}"#)
            .unwrap();
        assert_eq!(preview.command, "date");
        assert_eq!(preview.clear_rows, 4);
    }

    #[test]
    fn extracts_unclosed_command_preview() {
        let mut block = StreamingCommandBlock::new();

        let preview = block
            .command_from_progress("run_command", r#"{"command":"printf"#)
            .unwrap();
        assert_eq!(preview.command, "printf");
    }

    #[test]
    fn take_rendered_rows_resets_preview_state() {
        let mut block = StreamingCommandBlock::new();

        let _ = block.command_from_progress("run_command", r#"{"command":"printf"#);
        block.mark_rendered_rows(3);

        assert_eq!(block.take_rendered_rows(), 3);
        assert_eq!(block.take_rendered_rows(), 0);
    }
}
