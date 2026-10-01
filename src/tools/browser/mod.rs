//! 内置浏览器工具组：打开网页、结构快照、点击输入、滚动截图与脚本求值。
//!
//! 所有工具共用 `crate::browser` 的进程级浏览器会话，Web 工作台浏览器面板
//! 展示的是同一个页面，用户可以随时接手或观察 Agent 的操作。

mod capture;
mod interact;
mod page;

#[cfg(test)]
mod tests;

use super::ToolRegistry;
use crate::browser::BrowserSession;
use anyhow::{bail, Result};
use serde_json::Value;
use std::sync::Arc;

/// 浏览器工具组全部工具名，分组、展示名与测试共用。
pub(crate) const BROWSER_TOOL_NAMES: &[&str] = &[
    "browser_navigate",
    "browser_tabs",
    "browser_wait",
    "browser_snapshot",
    "browser_screenshot",
    "browser_click",
    "browser_type",
    "browser_select_option",
    "browser_press_key",
    "browser_scroll",
    "browser_evaluate",
];

/// 【浏览器工具】【工具注册】注册内置浏览器工具组。
/// @param registry 为工具注册表
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry) {
    page::register(registry);
    capture::register(registry);
    interact::register(registry);
}

/// 【浏览器工具】【会话获取】取得共享浏览器会话并广播本次操作摘要。
/// @param activity 为面板展示的操作摘要
/// @returns 浏览器会话
pub(super) async fn session_for(activity: &str) -> Result<Arc<BrowserSession>> {
    let session = crate::browser::shared().await?;
    session.emit_activity(activity);
    Ok(session)
}

/// 【浏览器工具】【必填字符串】读取非空字符串参数。
/// @param args 为工具参数；key 为字段名
/// @returns 去除首尾空白的字段值
pub(super) fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    match args.get(key).and_then(Value::as_str).map(str::trim) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => bail!("{key} is required"),
    }
}

/// 【浏览器工具】【可选字符串】读取可选的非空字符串参数。
/// @param args 为工具参数；key 为字段名
/// @returns 字段值；缺省或为空时为空
pub(super) fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// 【浏览器工具】【布尔参数】读取布尔参数。
/// @param args 为工具参数；key 为字段名；default 为缺省值
/// @returns 字段值
pub(super) fn bool_arg(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// 【浏览器工具】【数值参数】读取数值参数并限制在区间内。
/// @param args 为工具参数；key 为字段名；default 为缺省值；min、max 为区间
/// @returns 限制后的数值
pub(super) fn number_arg(args: &Value, key: &str, default: f64, min: f64, max: f64) -> f64 {
    args.get(key)
        .and_then(Value::as_f64)
        .unwrap_or(default)
        .clamp(min, max)
}
