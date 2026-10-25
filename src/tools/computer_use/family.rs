//! 分动作入口采用相同的会话、权限和快照，不创建第二套原生驱动。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 【桌面操作】【工具族】仅暴露窗口定向能力，不提供全局输入、启动应用或剪贴板写入。
pub(super) const TOOLS: &[(&str, &str, &str)] = &[
    ("computer_status", "status", "Inspect desktop-control configuration without starting a driver."),
    ("computer_apps", "list_apps", "List running applications; do not guess the frontmost target."),
    ("computer_windows", "list_windows", "List windows of an explicitly selected running app."),
    ("computer_screenshot", "get_app_state", "Capture one explicit window with a frame id, optional region and downscale. Coordinates refer to the delivered image, never the desktop."),
    ("computer_ui", "get_app_state", "Read an explicit window's accessibility tree and snapshot-bound element handles without capturing an image."),
    ("computer_click", "click", "Click a current element handle or image point in the observed window. Consumes the snapshot."),
    ("computer_drag", "drag", "Drag between two points in the current delivered window image. Consumes the snapshot."),
    ("computer_scroll", "scroll", "Scroll the observed window or its current element. Consumes the snapshot."),
    ("computer_type", "type_text", "Type text into the explicitly observed window or element. Consumes the snapshot."),
    ("computer_key", "press_key", "Send a key with optional modifiers to the observed window. Consumes the snapshot."),
    ("computer_ui_action", "ui_action", "Query current element text/value, locate text, invoke a control, show its menu or set its value. Mutations consume the snapshot; queries never click automatically."),
    ("computer_stop", "reset", "Close only this Computer Use session and invalidate its frames and element handles."),
];

/// 【桌面操作】【控件动作】无障碍入口只接受已实现且绑定观察的动作。
const UI_ACTIONS: &[&str] = &[
    "invoke",
    "show_menu",
    "set_value",
    "get_value",
    "get_text",
    "locate_text",
];

/// 【桌面操作】【动作查找】按精确工具名查找唯一的规范动作，不接受前缀猜测。
/// 参数: `name` 为工具名；返回: 对应动作，未知名称时返回错误。
fn action(name: &str) -> Result<&'static str> {
    TOOLS
        .iter()
        .find(|tool| tool.0 == name)
        .map(|tool| tool.1)
        .context("unknown Computer Use tool")
}

/// 【桌面操作】【分动作契约】复用统一动作 Schema，只公开当前入口允许的字段。
/// 参数: `name` 为分动作工具名；返回: 模型可见的严格参数 Schema。
pub(super) fn parameters(name: &str) -> Value {
    // 1. 【桌面操作】【动作范围】无障碍入口保留受限动作集合，其他入口仅公开固定动作字段
    let canonical = action(name).unwrap();
    let mut schema = super::schema::parameters();
    if canonical == "ui_action" {
        schema["properties"]["action"]["enum"] = json!(UI_ACTIONS);
        schema["oneOf"].as_array_mut().unwrap().retain(|branch| {
            branch["properties"]["action"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| UI_ACTIONS.contains(&value.as_str().unwrap()))
        });
        return schema;
    }
    let branch = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| {
            branch["properties"]["action"]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == canonical)
        })
        .unwrap()
        .clone();
    schema.as_object_mut().unwrap().remove("oneOf");
    schema["properties"]
        .as_object_mut()
        .unwrap()
        .retain(|field, _| field != "action" && branch["properties"].get(field).is_some());
    schema["required"] = branch["required"].clone();
    if matches!(name, "computer_screenshot" | "computer_ui") {
        schema["properties"]
            .as_object_mut()
            .unwrap()
            .remove("include_screenshot");
    }
    if name == "computer_ui" {
        for field in ["scale", "region", "max_image_dimension"] {
            schema["properties"].as_object_mut().unwrap().remove(field);
        }
    }
    // 2. 【桌面操作】【帧别名】坐标入口要求快照或帧证明，不降低目标校验要求
    if schema["properties"].get("snapshot_id").is_some() {
        schema["properties"]["from_frame"] = json!({"type":"string","minLength":1,"maxLength":128,
            "description":"Frame id from the current screenshot; an alias of snapshot_id, valid only for that app/window and consumed by one input."});
        schema["required"]
            .as_array_mut()
            .unwrap()
            .retain(|field| field != "snapshot_id");
        schema["anyOf"] = json!([{"required":["snapshot_id"]},{"required":["from_frame"]}]);
    }
    schema
}

/// 【桌面操作】【动作归一】先验证公开契约，再转换帧别名和固定观察类型，禁止覆盖入口动作。
/// 参数: `name` 为入口名，`args` 为模型参数；返回: 可交给现有会话的规范参数。
pub(super) fn normalize(name: &str, mut args: Value) -> Result<Value> {
    // 1. 【桌面操作】【入口校验】先拒绝未公开字段和动作覆盖，再转换到统一会话契约
    jsonschema::validator_for(&parameters(name))?
        .validate(&args)
        .map_err(|error| anyhow::anyhow!("invalid {name} arguments: {error}"))?;
    let canonical = action(name)?;
    let fields = args
        .as_object_mut()
        .context("Computer Use arguments must be an object")?;
    // 2. 【桌面操作】【观察绑定】两个别名同时提供时必须指向同一份观察
    if let Some(frame) = fields.remove("from_frame") {
        if fields
            .get("snapshot_id")
            .is_some_and(|snapshot| snapshot != &frame)
        {
            bail!("from_frame and snapshot_id must refer to the same current observation");
        }
        fields.insert("snapshot_id".into(), frame);
    }
    if canonical != "ui_action" {
        fields.insert("action".into(), json!(canonical));
    }
    if matches!(name, "computer_screenshot" | "computer_ui") {
        fields.insert(
            "include_screenshot".into(),
            json!(name == "computer_screenshot"),
        );
    }
    Ok(args)
}
