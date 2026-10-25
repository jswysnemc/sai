//! 对原生控件字段进行结构化限长，观察和后续查询共享同一份有界数据。

use serde_json::{Map, Value};

/// 【桌面操作】【快照字段】保留允许的控件字段及其类型，过长文本截断后仍然形成有效 JSON。
/// 参数: `element` 为驱动控件或窗口元数据，`keys` 为允许字段；返回: 有界字段与必要的截断标志。
pub(super) fn fields(element: &Value, keys: &[&str]) -> Map<String, Value> {
    // 1. 【桌面操作】【字段筛选】限制公开字段类型、字符串长度及动作列表数量
    let mut output = Map::new();
    let mut truncated = false;
    for key in keys {
        let Some(value) = element.get(*key) else {
            continue;
        };
        let bounded = match value {
            Value::String(text) => Value::String(limit_string(text, 256, &mut truncated)),
            Value::Bool(_) | Value::Number(_) | Value::Null => value.clone(),
            Value::Array(actions) if *key == "actions" => {
                truncated |= actions.len() > 8;
                Value::Array(
                    actions
                        .iter()
                        .take(8)
                        .filter_map(Value::as_str)
                        .map(|action| Value::String(limit_string(action, 32, &mut truncated)))
                        .collect(),
                )
            }
            _ => {
                truncated = true;
                continue;
            }
        };
        output.insert((*key).into(), bounded);
    }
    // 2. 【桌面操作】【截断标记】结构保留有效 JSON，查询和渲染均能识别信息不完整
    if truncated {
        output.insert("truncated".into(), Value::Bool(true));
    }
    output
}

/// 【桌面操作】【字段限长】按 Unicode 字符保留前缀，不截断 UTF-8 编码。
/// 参数: `text` 为原文，`limit` 为字符上限，`truncated` 为累计标志；返回: 有界字符串。
fn limit_string(text: &str, limit: usize, truncated: &mut bool) -> String {
    let mut characters = text.chars();
    let output = characters.by_ref().take(limit).collect::<String>();
    if characters.next().is_some() {
        *truncated = true;
    }
    output
}
