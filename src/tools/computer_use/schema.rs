//! 原生桌面工具的动作契约；目标、快照与坐标均显式声明。

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

/// 【桌面操作】【参数契约】构造统一工具的模型可见参数。
/// @returns 根节点为 object 的动作 Schema
pub(super) fn parameters() -> Value {
    json!({
        "type":"object",
        "properties": {
            "action":{"type":"string","enum":[
                "status","reset","list_apps","list_windows","get_app_state",
                "click","scroll","drag","press_key","type_text","set_value"
            ]},
            "app":{"type":"string","minLength":1,"maxLength":512,
                "description":"Explicit target: a positive process id from list_apps, or an exact, unambiguous running app name/bundle_id. Never defaults to the frontmost app."},
            "window_id":{"type":"integer","minimum":1,"maximum":9007199254740991_u64,
                "description":"Exact native window id from list_windows. Required for observation and input."},
            "snapshot_id":{"type":"string","minLength":1,"maxLength":128,
                "description":"Copy from the latest get_app_state for this app/window. Each input consumes the snapshot; observe again before another input."},
            "element_index":{"type":"string","maxLength":64,"pattern":"^g[0-9]+:[0-9]+$",
                "description":"Opaque handle copied from the same snapshot, for example g17:4. Do not invent handles or pass bare integers."},
            "x":coordinate("x"), "y":coordinate("y"),
            "from_x":coordinate("drag start x"), "from_y":coordinate("drag start y"),
            "to_x":coordinate("drag end x"), "to_y":coordinate("drag end y"),
            "button":{"type":"string","enum":["left","middle","right"]},
            "click_count":{"type":"integer","minimum":1,"maximum":3},
            "direction":{"type":"string","enum":["up","down","left","right"]},
            "amount":{"type":"integer","minimum":1,"maximum":100,
                "description":"Scroll distance in lines or pages, default 3."},
            "by":{"type":"string","enum":["line","page"]},
            "duration_ms":{"type":"integer","minimum":50,"maximum":5000},
            "text":{"type":"string","maxLength":65536},
            "value":{"type":["string","number"],"maxLength":65536},
            "key":{"type":"string","minLength":1,"maxLength":128,
                "description":"Native key name, such as Return, Tab or Escape. Supply modifiers separately."},
            "modifiers":{"type":"array","maxItems":4,"uniqueItems":true,
                "items":{"type":"string","enum":["ctrl","alt","shift","super","cmd","option","meta"]}},
            "query":{"type":"string","maxLength":512},
            "max_elements":{"type":"integer","minimum":1,"maximum":2000},
            "include_screenshot":{"type":"boolean","description":"Defaults to true. Without a screenshot, only element/keyboard actions are available."}
        },
        "required":["action"],
        "additionalProperties":false,
        "oneOf":[
            shape(&["status","reset","list_apps"], &[], &[]),
            shape(&["list_windows"], &["app"], &[]),
            shape(&["get_app_state"], &["app","window_id"], &["query","max_elements","include_screenshot"]),
            input_shape("click", &[], &["element_index","x","y","button","click_count","modifiers"]),
            input_shape("scroll", &["direction"], &["element_index","x","y","amount","by"]),
            input_shape("drag", &["from_x","from_y","to_x","to_y"], &["button","duration_ms"]),
            input_shape("press_key", &["key"], &["modifiers","element_index","x","y"]),
            input_shape("type_text", &["text"], &["element_index","x","y"]),
            input_shape("set_value", &["element_index","value"], &[])
        ]
    })
}

/// 【桌面操作】【坐标契约】声明最近一次窗口截图中的像素坐标。
/// @param label 为坐标用途
/// @returns 非负数值契约
fn coordinate(label: &str) -> Value {
    json!({"type":"number","minimum":0,
        "description":format!("{label} in the latest window screenshot's pixels, not desktop coordinates.")})
}

/// 【桌面操作】【动作字段】限定单组动作的字段与必填项。
/// @param actions 为动作名；required 为必填字段；optional 为可选字段
/// @returns oneOf 分支
fn shape(actions: &[&str], required: &[&str], optional: &[&str]) -> Value {
    let mut properties = Map::new();
    properties.insert("action".into(), json!({"enum":actions}));
    for field in required.iter().chain(optional) {
        properties.insert((*field).into(), json!({}));
    }
    json!({"properties":properties,"required":required,"additionalProperties":false})
}

/// 【桌面操作】【输入目标】为每个输入动作添加不可省略的目标与快照。
/// @param action 为动作名；required、optional 为动作专用字段
/// @returns 带统一输入边界的 Schema 分支
fn input_shape(action: &str, required: &[&str], optional: &[&str]) -> Value {
    let mut required_fields = vec!["app", "window_id", "snapshot_id"];
    required_fields.extend(required);
    shape(&[action], &required_fields, optional)
}

/// 【桌面操作】【参数校验】先完成纯参数校验，再允许查询应用或发送输入。
/// @param args 为工具参数
/// @returns 参数有效时成功，否则返回可修正的错误
pub(super) fn validate(args: &Value) -> Result<()> {
    jsonschema::validator_for(&parameters())?
        .validate(args)
        .map_err(|error| anyhow::anyhow!("{error}"))
        .context("invalid computer_use arguments")?;
    let point = args.get("x").is_some() || args.get("y").is_some();
    if point && (args.get("x").is_none() || args.get("y").is_none()) {
        bail!("x and y must be supplied together");
    }
    if point && args.get("element_index").is_some() {
        bail!("use element_index OR x/y, not both");
    }
    if args["action"] == "click" && !point && args.get("element_index").is_none() {
        bail!("click requires element_index or both x and y");
    }
    if args
        .get("app")
        .and_then(Value::as_str)
        .is_some_and(|app| app.trim().is_empty())
    {
        bail!("app must not be blank");
    }
    Ok(())
}

/// 【桌面操作】【整数读取】兼容 JSON Schema 允许的 101.0，拒绝越界或带小数的值。
/// @param args 为参数对象；key 为已声明整数类型的字段
/// @returns 可安全传给原生驱动的无符号整数
pub(super) fn integer(args: &Value, key: &str) -> Result<u64> {
    if let Some(value) = args.get(key).and_then(Value::as_u64) {
        return Ok(value);
    }
    if let Some(value) = args.get(key).and_then(Value::as_f64) {
        if value.is_finite()
            && value.fract() == 0.0
            && (0.0..=9_007_199_254_740_991.0).contains(&value)
        {
            return Ok(value as u64);
        }
    }
    bail!("{key} must be a non-negative integer")
}
