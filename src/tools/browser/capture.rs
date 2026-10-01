//! 读取类浏览器工具：结构快照、截图与脚本求值。

use super::{bool_arg, number_arg, optional_str, required_str, session_for};
use crate::browser::{DEFAULT_SNAPSHOT_CHARS, MAX_SNAPSHOT_CHARS};
use crate::tools::file_read::image_output;
use crate::tools::ToolOutput;
use anyhow::Result;
use serde_json::Value;

/// 脚本求值结果交给模型的最大字符数。
const EVALUATE_MAX_CHARS: usize = 20_000;

/// 【浏览器工具】【结构快照】提取当前页面结构树。
/// @param args 为工具参数
/// @returns 快照文本
pub(super) async fn snapshot(args: Value) -> Result<String> {
    let max_chars = number_arg(
        &args,
        "max_chars",
        DEFAULT_SNAPSHOT_CHARS as f64,
        2_000.0,
        MAX_SNAPSHOT_CHARS as f64,
    ) as usize;
    let interactive_only = bool_arg(&args, "interactive_only", false);
    let session = session_for("Reading page structure").await?;
    Ok(session
        .snapshot(max_chars, interactive_only)
        .await?
        .render())
}

/// 【浏览器工具】【页面截图】截图并作为图片附件交给模型。
/// @param args 为工具参数
/// @returns 带图片附件的工具结果
pub(super) async fn screenshot(args: Value) -> Result<ToolOutput> {
    let reference = optional_str(&args, "ref");
    let full_page = bool_arg(&args, "full_page", false);
    let session = session_for("Taking screenshot").await?;
    // 1. 截图后按读图工具同样的规则压缩，并写明截图范围与页面地址
    let (bytes, scope) = session.screenshot(full_page, reference).await?;
    let url = session.last_state().url;
    let mut output = image_output(&bytes, &format!("browser screenshot ({scope}) of {url}"))?;
    output.content.push_str(
        "\nCoordinates in this image are page CSS pixels when not resized; use browser with action=snapshot refs to interact.",
    );
    Ok(output)
}

/// 【浏览器工具】【脚本求值】执行表达式并截断过长结果。
/// @param args 为工具参数
/// @returns JSON 文本结果
pub(super) async fn evaluate(args: Value) -> Result<String> {
    let expression = required_str(&args, "expression")?;
    let session = session_for("Running script").await?;
    let value = session.evaluate(expression).await?;
    let mut text = match &value {
        Value::String(text) => text.clone(),
        Value::Null => "null (the expression returned undefined or null)".to_string(),
        other => serde_json::to_string_pretty(other)?,
    };
    if text.chars().count() > EVALUATE_MAX_CHARS {
        text = text.chars().take(EVALUATE_MAX_CHARS).collect();
        text.push_str("\n[result truncated]");
    }
    Ok(text)
}
