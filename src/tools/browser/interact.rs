//! 交互类浏览器工具：点击、输入、下拉选择、按键与滚动。

use super::{bool_arg, number_arg, optional_str, required_str, session_for};
use crate::browser::{ClickKind, ScrollDirection};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 【浏览器工具】【交互工具注册】注册点击、输入、选择、按键与滚动工具。
/// @param registry 为工具注册表
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new(
            "browser_click",
            "Click, double-click, right-click or hover an element by its ref from browser_snapshot. Uses real mouse events at the element center.",
            json!({
                "type": "object",
                "properties": {
                    "ref": {"type": "string", "description": "Element ref, e.g. e12."},
                    "action": {"type": "string", "enum": ["click", "double_click", "right_click", "hover"], "description": "Defaults to click."}
                },
                "required": ["ref"],
                "additionalProperties": false
            }),
            |args| async move { click(args).await },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "browser_type",
            "Type text into an input, textarea or contenteditable element by ref. Replaces the current value unless clear is false. Set submit to press Enter afterwards.",
            json!({
                "type": "object",
                "properties": {
                    "ref": {"type": "string", "description": "Element ref, e.g. e7."},
                    "text": {"type": "string", "description": "Text to type; any language is supported."},
                    "clear": {"type": "boolean", "description": "Replace existing content first; defaults to true."},
                    "submit": {"type": "boolean", "description": "Press Enter after typing."}
                },
                "required": ["ref", "text"],
                "additionalProperties": false
            }),
            |args| async move { type_text(args).await },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "browser_select_option",
            "Choose an option in a native <select> element by its visible text or value. For custom dropdowns, click them and click the option instead.",
            json!({
                "type": "object",
                "properties": {
                    "ref": {"type": "string", "description": "Ref of the select element."},
                    "option": {"type": "string", "description": "Option text or value."}
                },
                "required": ["ref", "option"],
                "additionalProperties": false
            }),
            |args| async move { select_option(args).await },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "browser_press_key",
            "Press a key or key combination in the focused element, e.g. Enter, Escape, Tab, ArrowDown, PageDown, Control+A, Shift+Tab.",
            json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string", "description": "Key name or combination joined with +."}
                },
                "required": ["key"],
                "additionalProperties": false
            }),
            |args| async move { press_key(args).await },
        )
        .writes(),
    );
    registry.register(ToolSpec::new(
        "browser_scroll",
        "Scroll the page, or a scrollable element by ref. With a ref and no direction, scroll that element into view.",
        json!({
            "type": "object",
            "properties": {
                "direction": {"type": "string", "enum": ["up", "down", "left", "right"], "description": "Scroll direction."},
                "amount": {"type": "number", "description": "Distance in CSS pixels, default 600."},
                "ref": {"type": "string", "description": "Element to scroll inside, or to bring into view."}
            },
            "additionalProperties": false
        }),
        |args| async move { scroll(args).await },
    ));
}

/// 【浏览器工具】【元素点击】按动作执行点击或悬停。
/// @param args 为工具参数
/// @returns 操作结果
async fn click(args: Value) -> Result<String> {
    let reference = required_str(&args, "ref")?;
    let action = optional_str(&args, "action").unwrap_or("click");
    let session = session_for(&format!("{action} {reference}")).await?;
    let outcome = match action {
        "click" => session.click(reference, ClickKind::Single).await?,
        "double_click" => session.click(reference, ClickKind::Double).await?,
        "right_click" => session.click(reference, ClickKind::Right).await?,
        "hover" => session.hover(reference).await?,
        other => bail!("unknown click action: {other}"),
    };
    Ok(outcome.describe())
}

/// 【浏览器工具】【文本输入】向元素输入文本。
/// @param args 为工具参数
/// @returns 操作结果
async fn type_text(args: Value) -> Result<String> {
    let reference = required_str(&args, "ref")?;
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("text is required"))?;
    let clear = bool_arg(&args, "clear", true);
    let submit = bool_arg(&args, "submit", false);
    let session = session_for(&format!("Typing into {reference}")).await?;
    Ok(session
        .type_text(reference, text, clear, submit)
        .await?
        .describe())
}

/// 【浏览器工具】【下拉选择】选中原生下拉框选项。
/// @param args 为工具参数
/// @returns 操作结果
async fn select_option(args: Value) -> Result<String> {
    let reference = required_str(&args, "ref")?;
    let option = required_str(&args, "option")?;
    let session = session_for(&format!("Selecting {option}")).await?;
    Ok(session.select_option(reference, option).await?.describe())
}

/// 【浏览器工具】【按键】按下按键或组合键。
/// @param args 为工具参数
/// @returns 操作结果
async fn press_key(args: Value) -> Result<String> {
    let key = required_str(&args, "key")?;
    let session = session_for(&format!("Pressing {key}")).await?;
    session.press_key(key).await?;
    let page = session
        .settle(crate::browser::navigation::SETTLE_TIMEOUT)
        .await?;
    Ok(format!("Pressed {key}.\n{}", page.describe()))
}

/// 【浏览器工具】【页面滚动】滚动页面、元素内部或把元素滚入视口。
/// @param args 为工具参数
/// @returns 滚动结果
async fn scroll(args: Value) -> Result<String> {
    let reference = optional_str(&args, "ref");
    let direction = optional_str(&args, "direction");
    let session = session_for("Scrolling").await?;
    match (direction, reference) {
        (None, Some(reference)) => session.scroll_into_view(reference).await,
        (direction, reference) => {
            let direction = ScrollDirection::parse(direction.unwrap_or("down"))?;
            let amount = number_arg(&args, "amount", 600.0, 1.0, 20_000.0);
            session.scroll(direction, amount, reference).await
        }
    }
}
