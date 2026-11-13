//! 兼容事件名称、标准输入、匹配器及 MCP 参数模板。

use super::{HookContext, HookEvent};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 【Hooks】【事件输入】参数为事件和上下文，返回 snake_case 字段的 JSON 输入
pub(super) fn input(event: HookEvent, context: &HookContext) -> Value {
    let mut value = json!({"hook_event_name":event.as_str(),"session_id":context.session_id,
        "cwd":context.workdir,"transcript_path":null});
    if let Some(name) = &context.tool_name {
        value["tool_name"] = json!(name);
    }
    for (key, text) in &context.extra {
        let field = match key.as_str() {
            "SAI_TURN_ID" => "turn_id",
            "SAI_TOOL_ARGUMENTS" => "tool_input",
            "SAI_TOOL_RESULT" => "tool_response",
            key if key.starts_with("SAI_") => {
                let _ = key;
                continue;
            }
            key => key,
        };
        value[field] = serde_json::from_str(text).unwrap_or_else(|_| json!(text));
    }
    value
}

/// 【Hooks】【匹配器】参数为匹配表达式与事件输入，返回是否命中，无效正则返回错误
pub(super) fn matches(matcher: Option<&str>, event: HookEvent, input: &Value) -> Result<bool> {
    let pattern = matcher.unwrap_or("").trim();
    if pattern.is_empty() || pattern == "*" {
        return Ok(true);
    }
    let field = match event {
        HookEvent::PreToolUse | HookEvent::PostToolUse | HookEvent::PermissionRequest => {
            "tool_name"
        }
        HookEvent::SessionStart => "source",
        HookEvent::SessionEnd => "reason",
        HookEvent::PreCompact | HookEvent::PostCompact => "trigger",
        HookEvent::SubagentStart | HookEvent::SubagentStop => "agent_type",
        _ => return Ok(true),
    };
    let name = input[field].as_str().unwrap_or_default();
    let mut names = vec![name];
    match name {
        "run_command" | "background_command" => names.push("Bash"),
        "read_file" => names.push("Read"),
        "write_file" => names.push("Write"),
        "apply_patch" | "str_replace" => names.extend(["Edit", "Write"]),
        "grep" => names.push("Grep"),
        "glob" => names.push("Glob"),
        _ => {}
    }
    if pattern
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '|'))
    {
        return Ok(pattern.split('|').any(|pattern| names.contains(&pattern)));
    }
    let regex = regex::Regex::new(pattern).context("invalid hook matcher")?;
    Ok(names.iter().any(|name| regex.is_match(name)))
}

/// 【Hooks】【MCP 模板】递归替换占位符；参数为模板与事件输入，返回保留类型的工具参数
pub(super) fn expand(template: &Value, input: &Value) -> Result<Value> {
    Ok(match template {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| Ok((key.clone(), expand(value, input)?)))
                .collect::<Result<_>>()?,
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| expand(value, input))
                .collect::<Result<_>>()?,
        ),
        Value::String(text) => {
            let regex = regex::Regex::new(r"\$\{([^}]+)\}")?;
            let lookup = |field: &str| -> Result<Value> {
                let mut value = input;
                for part in field.split('.') {
                    value = value
                        .get(part)
                        .with_context(|| format!("missing hook field: {field}"))?;
                }
                Ok(value.clone())
            };
            if let Some(capture) = regex
                .captures(text)
                .filter(|capture| capture[0].len() == text.len())
            {
                lookup(&capture[1])?
            } else {
                let mut output = String::new();
                let mut cursor = 0;
                for capture in regex.captures_iter(text) {
                    let span = capture.get(0).context("hook placeholder")?;
                    output.push_str(&text[cursor..span.start()]);
                    let value = lookup(&capture[1])?;
                    output.push_str(value.as_str().unwrap_or(&value.to_string()));
                    cursor = span.end();
                }
                output.push_str(&text[cursor..]);
                Value::String(output)
            }
        }
        other => other.clone(),
    })
}

/// 【Hooks】【协议校验】参数为命令输出与事件，返回 JSON；允许注入上下文的事件兼容纯文本
pub(super) fn output(text: &str, event: HookEvent) -> Result<Value> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(value)) => Ok(Value::Object(value)),
        _ if matches!(
            event,
            HookEvent::SessionStart | HookEvent::UserPromptSubmit | HookEvent::SubagentStart
        ) && !text.trim_start().starts_with(['{', '[']) =>
        {
            Ok(
                json!({"hookSpecificOutput":{"hookEventName":event.as_str(),"additionalContext":text}}),
            )
        }
        _ => bail!("hook stdout must contain a single JSON object"),
    }
}
