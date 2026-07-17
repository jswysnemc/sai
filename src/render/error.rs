use crate::i18n::text as t;
use crate::render::style::{ASSET_ERROR_STYLE, RESET, TOOL_BULLET};
use anyhow::{Error, Result};
use serde_json::Value;
use std::io::{self, Write};

/// 输出聊天失败错误。
///
/// 参数:
/// - `error`: 需要展示给用户的错误
/// - `plain`: 是否使用纯文本输出
///
/// 返回:
/// - 输出是否成功
pub(crate) fn write_chat_error(error: &Error, plain: bool) -> Result<()> {
    let mut stdout = io::stdout();
    let lines = error_chain_lines(error);
    let title = t("request failed", "请求失败");
    if plain {
        writeln!(stdout, "{title}: {}", lines.join(": "))?;
        return Ok(());
    }
    writeln!(stdout, "{ASSET_ERROR_STYLE}{TOOL_BULLET} {title}{RESET}")?;
    for line in lines {
        writeln!(stdout, "  {ASSET_ERROR_STYLE}{line}{RESET}")?;
    }
    Ok(())
}

/// 提取错误链路文本。
///
/// 参数:
/// - `error`: 原始错误
///
/// 返回:
/// - 去重后的错误链路
fn error_chain_lines(error: &Error) -> Vec<String> {
    let mut lines = Vec::new();
    for item in error.chain() {
        let text = simplify_error_text(&item.to_string());
        if text.trim().is_empty() {
            continue;
        }
        if lines.last() == Some(&text) {
            continue;
        }
        lines.push(text);
    }
    if lines.is_empty() {
        lines.push(t("unknown error", "未知错误").to_string());
    }
    lines
}

/// 简化 API 错误文本。
///
/// 参数:
/// - `text`: 原始错误文本
///
/// 返回:
/// - 更适合终端展示的错误文本
fn simplify_error_text(text: &str) -> String {
    let Some((prefix, body)) = text.split_once(": {") else {
        return text.to_string();
    };
    let body = format!("{{{body}");
    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return text.to_string();
    };
    let Some(error) = value.get("error") else {
        return text.to_string();
    };
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(t("unknown provider error", "未知供应商错误"));
    let kind = error.get("type").and_then(Value::as_str);
    let code = error.get("code").and_then(Value::as_str);
    let mut details = Vec::new();
    if let Some(kind) = kind.filter(|value| !value.is_empty()) {
        details.push(format!("type={kind}"));
    }
    if let Some(code) = code.filter(|value| !value.is_empty()) {
        details.push(format!("code={code}"));
    }
    if details.is_empty() {
        format!("{prefix}: {message}")
    } else {
        format!("{prefix}: {message} ({})", details.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_chain_lines_deduplicates_adjacent_messages() {
        let error = anyhow::anyhow!("api failed");
        assert_eq!(error_chain_lines(&error), vec!["api failed"]);
    }

    #[test]
    fn simplify_error_text_extracts_openai_error_body() {
        let text = r#"chat completions stream request failed (402 Payment Required): {"error":{"message":"Insufficient Balance","type":"unknown_error","param":null,"code":"invalid_request_error"}}"#;
        assert_eq!(
            simplify_error_text(text),
            "chat completions stream request failed (402 Payment Required): Insufficient Balance (type=unknown_error, code=invalid_request_error)"
        );
    }
}
