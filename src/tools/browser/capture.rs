//! 读取类浏览器工具：结构快照、截图与脚本求值。

use super::{bool_arg, number_arg, optional_str, required_str, session_for};
use crate::browser::{DEFAULT_SNAPSHOT_CHARS, MAX_SNAPSHOT_CHARS};
use crate::tools::file_read::image_output;
use crate::tools::{ToolOutput, ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};

/// 脚本求值结果交给模型的最大字符数。
const EVALUATE_MAX_CHARS: usize = 20_000;

/// 【浏览器工具】【读取工具注册】注册快照、截图与脚本求值工具。
/// @param registry 为工具注册表
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "browser_snapshot",
        "Read the current page as an accessibility-style tree. Interactive elements get refs like [ref=e12] that browser_click, browser_type and the other tools accept. Refs are reset on every snapshot and after navigation, so take a fresh snapshot after the page changes.",
        json!({
            "type": "object",
            "properties": {
                "interactive_only": {"type": "boolean", "description": "List only interactive elements, without page text. Use on very long pages."},
                "max_chars": {"type": "integer", "description": format!("Snapshot size limit in characters, default {DEFAULT_SNAPSHOT_CHARS}, max {MAX_SNAPSHOT_CHARS}.")}
            },
            "additionalProperties": false
        }),
        |args| async move { snapshot(args).await },
    ));
    registry.register(ToolSpec::new_with_output(
        "browser_screenshot",
        "Take a screenshot of the current page, the full page, or one element, and look at it directly. Use it when layout, images or visual state matter; use browser_snapshot to find refs.",
        json!({
            "type": "object",
            "properties": {
                "full_page": {"type": "boolean", "description": "Capture the whole scrollable page instead of the viewport."},
                "ref": {"type": "string", "description": "Capture only this element ref from browser_snapshot."}
            },
            "additionalProperties": false
        }),
        |args| async move { screenshot(args).await },
    ));
    registry.register(
        ToolSpec::new(
            "browser_evaluate",
            "Run a JavaScript expression in the current page and return its JSON value (promises are awaited). Use it to extract data the snapshot does not show. It runs with the page's privileges, so never use it to read secrets or submit anything the user did not ask for.",
            json!({
                "type": "object",
                "properties": {
                    "expression": {"type": "string", "description": "JavaScript expression, e.g. [...document.querySelectorAll('h2')].map(h => h.textContent)."}
                },
                "required": ["expression"],
                "additionalProperties": false
            }),
            |args| async move { evaluate(args).await },
        )
        .writes(),
    );
}

/// 【浏览器工具】【结构快照】提取当前页面结构树。
/// @param args 为工具参数
/// @returns 快照文本
async fn snapshot(args: Value) -> Result<String> {
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
async fn screenshot(args: Value) -> Result<ToolOutput> {
    let reference = optional_str(&args, "ref");
    let full_page = bool_arg(&args, "full_page", false);
    let session = session_for("Taking screenshot").await?;
    // 1. 截图后按读图工具同样的规则压缩，并写明截图范围与页面地址
    let (bytes, scope) = session.screenshot(full_page, reference).await?;
    let url = session.last_state().url;
    let mut output = image_output(&bytes, &format!("browser screenshot ({scope}) of {url}"))?;
    output.content.push_str(
        "\nCoordinates in this image are page CSS pixels when not resized; use browser_snapshot refs to interact.",
    );
    Ok(output)
}

/// 【浏览器工具】【脚本求值】执行表达式并截断过长结果。
/// @param args 为工具参数
/// @returns JSON 文本结果
async fn evaluate(args: Value) -> Result<String> {
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
