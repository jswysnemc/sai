use anyhow::{bail, Result};
use serde_json::Value;

/// 读取可选字符串参数。
///
/// 参数:
/// - `args`: JSON 参数
/// - `name`: 参数名称
///
/// 返回:
/// - 可选字符串
pub(super) fn optional_string_arg(args: &Value, name: &str) -> Result<Option<String>> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(text) = value.as_str() else {
        bail!("{name} must be a string");
    };
    Ok(Some(text.trim().to_string()))
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: JSON 参数
/// - `name`: 参数名称
///
/// 返回:
/// - 字符串参数
pub(super) fn string_arg(args: &Value, name: &str) -> Result<String> {
    let value = optional_string_arg(args, name)?;
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        bail!("missing required string argument: {name}");
    };
    Ok(value)
}

/// 从子智能体提示中生成短描述。
///
/// 参数:
/// - `prompt`: 子智能体提示
///
/// 返回:
/// - 短描述文本
pub(super) fn summarize_prompt(prompt: &str) -> String {
    let mut description = prompt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(super::DESCRIPTION_MAX_CHARS)
        .collect::<String>();
    if description.is_empty() {
        description = "subagent".to_string();
    }
    description
}
