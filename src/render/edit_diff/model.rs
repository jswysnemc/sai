use crate::render::terminal_text as t;
use crate::tools::edit_patch::AppliedPatch;
use anyhow::Result;
use serde_json::Value;

/// 根据 edit_file 参数构造 diff 预览。
///
/// 参数:
/// - `arguments`: edit_file 工具参数 JSON
///
/// 返回:
/// - 可渲染的 patch 预览
pub(crate) fn preview_from_arguments(arguments: &str) -> Result<AppliedPatch> {
    let value = match serde_json::from_str::<Value>(arguments) {
        Ok(value) => value,
        Err(err) => {
            if let Some(patch) = string_field_from_partial(arguments, "patch") {
                return crate::tools::edit_patch::preview_patch(
                    &patch,
                    &crate::runtime_cwd::current_dir()?,
                );
            }
            return Err(err.into());
        }
    };
    let patch = value
        .get("patch")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t("patch is required", "必须提供 patch")))?;
    crate::tools::edit_patch::preview_patch(patch, &crate::runtime_cwd::current_dir()?)
}

/// 从部分 JSON 参数中提取已闭合字符串字段。
///
/// 参数:
/// - `raw`: 原始 JSON 或 JSON 片段
/// - `key`: 字段名
///
/// 返回:
/// - 字符串字段内容，字段未闭合时返回空
fn string_field_from_partial(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let quote_index = after_colon.find('"')?;
    parse_json_string(&after_colon[quote_index..])
}

/// 解析 JSON 字符串片段。
///
/// 参数:
/// - `value`: 以双引号开头的 JSON 字符串片段
///
/// 返回:
/// - 解析后的字符串，未闭合时返回空
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
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_requires_patch_field() {
        let err = preview_from_arguments(r#"{"path":"a.rs","content":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("patch"));
    }
}
