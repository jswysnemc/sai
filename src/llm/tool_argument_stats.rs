/// 从（可能未闭合的）编辑工具参数流中近似统计增删行数。
///
/// 参数流阶段 JSON 尚未闭合，无法构建精确 diff 预览；这里按字段近似：
/// `str_replace` 的 old_string 计删、new_string 计增，`write_file` 的
/// content 全部按新增计。数字随分片增长，用于状态行实时跳动。
///
/// 参数:
/// - `arguments`: 工具参数原始文本（可能是不完整的 JSON 前缀）
///
/// 返回:
/// - `(新增行数, 删除行数)`；尚无可统计字段时返回空
pub(crate) fn streamed_diff_counts(arguments: &str) -> Option<(usize, usize)> {
    let removed = lenient_field_line_count(arguments, "old_string");
    let added = lenient_field_line_count(arguments, "new_string");
    if removed.is_some() || added.is_some() {
        return Some((added.unwrap_or(0), removed.unwrap_or(0)));
    }
    lenient_field_line_count(arguments, "content").map(|added| (added, 0))
}

/// 统计字段字符串值中已接收的行数。
///
/// 与严格 JSON 解析不同：值未闭合时统计已到达的部分，供流式跳动使用。
/// 以转义状态机单遍扫描，`\n` 转义计行、字面反斜杠不误计。
///
/// 值已闭合且以换行结尾时不补最后一行：`"l1\nl2\n"` 是两行而不是三行，
/// 否则徽标会比 diff 正文画出的行多一行。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 已接收行数；字段尚未出现时返回空，值为空字符串时返回 0
fn lenient_field_line_count(raw: &str, key: &str) -> Option<usize> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let value = after_colon.strip_prefix('"')?;
    let mut newlines = 0usize;
    let mut has_content = false;
    let mut ends_with_newline = false;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            if ch == 'n' {
                newlines += 1;
            }
            has_content = true;
            ends_with_newline = ch == 'n';
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => {
                // 值已闭合：末尾的换行是行终止符，后面不再有新行
                return Some(if !has_content {
                    0
                } else if ends_with_newline {
                    newlines
                } else {
                    newlines + 1
                });
            }
            _ => has_content = true,
        }
        ends_with_newline = false;
    }
    if !has_content {
        return Some(0);
    }
    // 值尚未闭合：最后一行还在增长，先按已到达的行数计
    Some(newlines + 1)
}
