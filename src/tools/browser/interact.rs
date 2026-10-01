//! 交互类浏览器工具：点击、输入、下拉选择、按键与滚动。

use super::{bool_arg, number_arg, optional_str, required_str, session_for};
use crate::browser::{ClickKind, ScrollDirection};
use anyhow::{bail, Result};
use serde_json::Value;

/// 【浏览器工具】【元素点击】按动作执行点击或悬停。
/// @param args 为工具参数
/// @returns 操作结果
pub(super) async fn click(args: Value) -> Result<String> {
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
pub(super) async fn type_text(args: Value) -> Result<String> {
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
pub(super) async fn select_option(args: Value) -> Result<String> {
    let reference = required_str(&args, "ref")?;
    let option = required_str(&args, "option")?;
    let session = session_for(&format!("Selecting {option}")).await?;
    Ok(session.select_option(reference, option).await?.describe())
}

/// 【浏览器工具】【按键】按下按键或组合键。
/// @param args 为工具参数
/// @returns 操作结果
pub(super) async fn press_key(args: Value) -> Result<String> {
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
pub(super) async fn scroll(args: Value) -> Result<String> {
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
