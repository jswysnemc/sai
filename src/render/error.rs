use crate::render::style::{ASSET_ERROR_STYLE, RESET, TOOL_BULLET};
use crate::render::terminal_text as t;
use anyhow::{Error, Result};
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
    // 错误走 stderr：stdout 可能被重定向为可解析输出，不能混入错误文本
    let mut stderr = io::stderr();
    let disconnect = crate::llm::is_transient_transport_error(error);
    let lines = error_chain_lines(error);
    let title = if disconnect {
        t("connection interrupted", "连接中断")
    } else {
        t("request failed", "请求失败")
    };
    if plain {
        writeln!(stderr, "{title}: {}", lines.join(": "))?;
        if disconnect {
            writeln!(
                stderr,
                "{}",
                t("You can retry this turn.", "可重试本轮请求。")
            )?;
        }
        return Ok(());
    }
    writeln!(stderr, "{ASSET_ERROR_STYLE}{TOOL_BULLET} {title}{RESET}")?;
    for line in lines {
        writeln!(stderr, "  {ASSET_ERROR_STYLE}{line}{RESET}")?;
    }
    if disconnect {
        writeln!(
            stderr,
            "  {ASSET_ERROR_STYLE}{}{RESET}",
            t("You can retry this turn.", "可重试本轮请求。")
        )?;
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
        let text = crate::llm::simplify_error_text(&item.to_string());
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
            crate::llm::simplify_error_text(text),
            "chat completions stream request failed (402 Payment Required): Insufficient Balance (type=unknown_error, code=invalid_request_error)"
        );
    }
}
