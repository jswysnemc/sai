//! 内置浏览器工具组：打开网页、结构快照、点击输入、滚动截图与脚本求值。
//!
//! 所有工具共用 `crate::browser` 的进程级浏览器会话，Web 工作台浏览器面板
//! 展示的是同一个页面，用户可以随时接手或观察 Agent 的操作。

mod capture;
mod interact;
mod page;
mod schema;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod round_trip_tests;

use super::{ToolOutput, ToolPermission, ToolRegistry, ToolSpec};
use crate::browser::BrowserSession;
use anyhow::{bail, Result};
use serde_json::Value;
use std::sync::Arc;

/// 单一公开入口，供分组与注册测试共用。
pub(crate) const BROWSER_TOOL_NAMES: &[&str] = &["browser"];

/// 【浏览器工具】【工具注册】注册统一入口，按 action 分派到内部实现。
/// @param registry 为工具注册表
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new_with_output(
            "browser",
            "Control the built-in browser shared with the user's Web workbench. Use action=navigate with url to open a page (http/https only), then action=snapshot to read its structure and obtain element refs. Use those refs for click/type/select_option; refs become stale after navigation or a new snapshot. action=screenshot returns an image for visual inspection. Navigation: navigate/back/forward/reload. Tabs: tabs/new_tab/switch_tab/close_tab. Other actions: wait/snapshot/screenshot/click/double_click/right_click/hover/type/select_option/press_key/scroll/evaluate. Calls can affect the same page; perform dependent actions in order. Evaluate executes a JavaScript expression with page privileges and awaits promises.",
            schema::parameters(),
            execute,
        )
        .writes()
        .with_call_permission(call_permission),
    );
}

/// 【浏览器工具】【动作权限】保持各操作合并前的只读或写入权限。
/// @param args 为工具参数
/// @returns 未知或缺少动作时按写入处理，避免降低权限
fn call_permission(args: &Value) -> ToolPermission {
    match args.get("action").and_then(Value::as_str) {
        Some(
            "navigate" | "back" | "forward" | "reload" | "tabs" | "new_tab" | "switch_tab"
            | "close_tab" | "wait" | "snapshot" | "screenshot" | "scroll",
        ) => ToolPermission::ReadOnly,
        _ => ToolPermission::Writes,
    }
}

/// 【浏览器工具】【操作分派】校验动作及其参数后调用内部操作，截图保留图片附件。
/// @param args 为统一工具参数
/// @returns 文本及可选图片附件
async fn execute(args: Value) -> Result<ToolOutput> {
    // 1. 【浏览器工具】【参数校验】直接调用注册表时也必须拒绝非法参数
    schema::validate(&args)?;
    let action = required_str(&args, "action")?;
    let text = match action {
        "navigate" | "back" | "forward" | "reload" => page::navigate(args).await?,
        "tabs" | "new_tab" | "switch_tab" | "close_tab" => page::tabs(args).await?,
        "wait" => page::wait(args).await?,
        "snapshot" => capture::snapshot(args).await?,
        "screenshot" => return capture::screenshot(args).await,
        "evaluate" => capture::evaluate(args).await?,
        "click" | "double_click" | "right_click" | "hover" => interact::click(args).await?,
        "type" => interact::type_text(args).await?,
        "select_option" => interact::select_option(args).await?,
        "press_key" => interact::press_key(args).await?,
        "scroll" => interact::scroll(args).await?,
        _ => bail!("unknown browser action: {action}"),
    };
    Ok(ToolOutput::text(text))
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
