//! 从当前有效快照提取输入目标名称，不根据句柄或坐标猜测控件。

use serde_json::Value;
use std::collections::HashMap;

/// 【桌面操作】【输入目标】使用已观测的角色与标签描述本次输入对象。
/// 参数: `args` 为输入参数，`elements` 为当前快照控件；返回: 可读名称，无标签时返回空。
pub(super) fn describe(args: &Value, elements: &HashMap<String, Value>) -> Option<String> {
    let handle = args["element_index"].as_str()?;
    let element = elements.get(handle)?;
    let label = element["label"].as_str()?.trim();
    if label.is_empty() {
        return None;
    }
    let role = element["role"].as_str().unwrap_or("control");
    Some(
        format!("{role} {label}")
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 【桌面操作】【目标回归】只使用当前快照名称，未知句柄不提供虚构标签。
    /// 参数: 无；返回: 无。
    #[test]
    fn input_target_comes_from_current_snapshot() {
        let elements = HashMap::from([("g1:0".into(), json!({"role":"button","label":"Apply"}))]);
        assert_eq!(
            describe(&json!({"element_index":"g1:0"}), &elements).as_deref(),
            Some("button Apply")
        );
        assert!(describe(&json!({"element_index":"g2:0"}), &elements).is_none());
        assert!(describe(&json!({"x":1,"y":2}), &elements).is_none());
    }
}
