//! 统一浏览器工具的参数契约：每个动作仅接受相关字段。

use crate::browser::{DEFAULT_SNAPSHOT_CHARS, MAX_SNAPSHOT_CHARS};
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

/// 【浏览器工具】【参数契约】构造模型可见的动作和参数定义。
/// @returns 根节点为 object、按 action 校验必填字段的 JSON Schema
pub(super) fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {"type":"string", "enum":[
                "navigate", "back", "forward", "reload", "tabs", "new_tab", "switch_tab", "close_tab",
                "wait", "snapshot", "screenshot", "click", "double_click", "right_click", "hover",
                "type", "select_option", "press_key", "scroll", "evaluate"
            ]},
            "url": {"type":"string", "minLength":1, "description":"Required for navigate; optional for new_tab (defaults to about:blank). Bare hosts are completed with https; localhost uses http."},
            "id": {"type":"string", "minLength":1, "description":"Tab id from tabs. Required for switch_tab; close_tab defaults to the current tab."},
            "ref": {"type":"string", "pattern":"^(ref=)?e[0-9]+$", "description":"Element ref from snapshot. Required for click/double_click/right_click/hover/type/select_option; optional for screenshot/scroll."},
            "text": {"type":"string", "description":"Text to enter for type (empty clears the field), or page text to await for wait."},
            "selector": {"type":"string", "minLength":1, "description":"For wait: CSS selector to await. With neither text nor selector, wait for seconds."},
            "seconds": {"type":"number", "minimum":1, "maximum":60, "description":"For wait: timeout or fixed delay in seconds, default 10."},
            "interactive_only": {"type":"boolean", "description":"For snapshot: omit non-interactive content, default false."},
            "max_chars": {"type":"integer", "minimum":2000, "maximum":MAX_SNAPSHOT_CHARS, "description":format!("For snapshot: character limit, default {DEFAULT_SNAPSHOT_CHARS}.")},
            "full_page": {"type":"boolean", "description":"For screenshot: capture the full page rather than viewport, default false."},
            "expression": {"type":"string", "minLength":1, "description":"For evaluate: JavaScript expression; returns its JSON value, awaiting promises."},
            "clear": {"type":"boolean", "description":"For type: replace existing contents first, default true."},
            "submit": {"type":"boolean", "description":"For type: press Enter afterwards, default false."},
            "option": {"type":"string", "minLength":1, "description":"For select_option: visible option text or value in a native select."},
            "key": {"type":"string", "minLength":1, "description":"For press_key: key or chord such as Enter, Escape, Control+A, Shift+Tab."},
            "direction": {"type":"string", "enum":["up","down","left","right"], "description":"For scroll: defaults to down; ref without direction brings the element into view."},
            "amount": {"type":"number", "minimum":1, "maximum":20000, "description":"For scroll: distance in CSS pixels, default 600."}
        },
        "required": ["action"],
        "additionalProperties": false,
        "oneOf": [
            shape(&["navigate"], &["url"], &[]),
            shape(&["back", "forward", "reload", "tabs"], &[], &[]),
            shape(&["new_tab"], &[], &["url"]),
            shape(&["switch_tab"], &["id"], &[]),
            shape(&["close_tab"], &[], &["id"]),
            shape(&["wait"], &[], &["text", "selector", "seconds"]),
            shape(&["snapshot"], &[], &["interactive_only", "max_chars"]),
            shape(&["screenshot"], &[], &["full_page", "ref"]),
            shape(&["evaluate"], &["expression"], &[]),
            shape(&["click", "double_click", "right_click", "hover"], &["ref"], &[]),
            shape(&["type"], &["ref", "text"], &["clear", "submit"]),
            shape(&["select_option"], &["ref", "option"], &[]),
            shape(&["press_key"], &["key"], &[]),
            shape(&["scroll"], &[], &["direction", "amount", "ref"])
        ]
    })
}

/// 【浏览器工具】【动作字段】声明一组动作允许使用的字段，字段类型由根契约统一校验。
/// @param actions 为动作名称；required 为必填字段；optional 为可选字段
/// @returns oneOf 中的单个分支
fn shape(actions: &[&str], required: &[&str], optional: &[&str]) -> Value {
    let mut properties = Map::new();
    properties.insert("action".into(), json!({"enum":actions}));
    for field in required.iter().chain(optional) {
        properties.insert((*field).into(), json!({}));
    }
    json!({"properties":properties, "required":required, "additionalProperties":false})
}

/// 【浏览器工具】【参数校验】拒绝未知动作、缺少必填字段和与动作不匹配的参数。
/// @param args 为工具调用参数
/// @returns 参数合法时成功，否则返回校验原因
pub(super) fn validate(args: &Value) -> Result<()> {
    let validator = jsonschema::validator_for(&parameters())?;
    validator
        .validate(args)
        .map_err(|error| anyhow::anyhow!("{error}"))
        .context("invalid browser action arguments")
}
