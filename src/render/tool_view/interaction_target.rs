//! 交互卡片的人类可读目标与输入动作分类，不根据技术标识猜测控件名称。

use serde_json::Value;

/// 【终端】【交互目标】优先使用明确目标描述和观测中的应用名称，缺失时保留技术兜底。
/// 参数: `args` 为工具参数，`output` 为可选观测文本；返回: 可读单行目标。
pub(super) fn desktop(args: &Value, output: Option<&str>) -> Option<String> {
    let observed = output.and_then(|output| {
        serde_json::from_str::<Value>(output).ok().or_else(|| {
            output
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|value| value.get("app_name").is_some())
        })
    });
    let name = output
        .and_then(|output| {
            output
                .lines()
                .find_map(|line| line.strip_prefix("Target: "))
        })
        .or_else(|| {
            observed
                .as_ref()
                .and_then(|value| value["app_name"].as_str())
        })
        .or_else(|| args["app"].as_str());
    name.filter(|name| !name.trim().is_empty()).map(|name| {
        let name = super::result_card::short(name, 48);
        if name.bytes().all(|byte| byte.is_ascii_digit()) {
            format!("app {name}")
        } else {
            name
        }
    })
}

/// 【终端】【输入分类】仅把会发送交互事件的动作标记为待验证输入。
/// 参数: `action` 为动作键；返回: 是否需要区分输入发送与效果验证。
pub(super) fn is_input(action: &str) -> bool {
    matches!(
        action,
        "click"
            | "double_click"
            | "right_click"
            | "drag"
            | "hover"
            | "scroll"
            | "type"
            | "type_text"
            | "press_key"
            | "set_value"
            | "select_option"
            | "invoke"
            | "show_menu"
    )
}
