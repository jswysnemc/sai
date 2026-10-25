use super::{accessibility, response, session::WindowTarget, snapshot_data};
use serde_json::{json, Value};
use std::collections::HashMap;

/// 【桌面测试】【结构限长】长字段仍可解析，查询复用限长数据，不能恢复超长原文。
/// 参数: 无；返回: 无。
#[test]
fn oversized_fields_remain_structured_and_bounded_in_queries() {
    let target = WindowTarget {
        pid: 42,
        window_id: 101,
    };
    let result = json!({"structuredContent":{"elements":[{
        "element_token":"private-token", "role":"text", "label":"匹配".repeat(10000),
        "value":"\"\\\n".repeat(10000), "actions":vec!["press"; 9]
    }]}});
    let observed = response::observation(&result, target, 1, "frame", &json!({})).unwrap();
    assert!(observed.output.content.len() < 10_000);
    let row = observed
        .output
        .content
        .lines()
        .find(|line| line.contains("element_index"))
        .unwrap();
    let row: Value = serde_json::from_str(row).unwrap();
    assert_eq!(row["truncated"], true);
    assert_eq!(row["actions"].as_array().unwrap().len(), 8);
    assert_eq!(observed.element_data["g1:0"], row);
    let query = accessibility::query(
        &json!({"action":"get_value","element_index":"g1:0"}),
        &observed.element_data,
    )
    .unwrap();
    assert_eq!(query["truncated"], true);
    assert!(query.to_string().len() < 4096);
    let markers = accessibility::query(
        &json!({"action":"locate_text","name":"truncated"}),
        &observed.element_data,
    )
    .unwrap();
    assert_eq!(markers["total_matches"], 0);
}

/// 【桌面测试】【匹配限长】大量命中保留顺序和总数，明确报告未展示的匹配项。
/// 参数: 无；返回: 无。
#[test]
fn locate_text_bounds_matches_and_reports_the_total() {
    let elements = (0..2000)
        .map(|index| {
            let handle = format!("g1:{index}");
            (
                handle,
                Value::Object(snapshot_data::fields(
                    &json!({"role":"text","label":"match".repeat(100)}),
                    &["role", "label"],
                )),
            )
        })
        .collect::<HashMap<_, _>>();
    let result =
        accessibility::query(&json!({"action":"locate_text","name":"match"}), &elements).unwrap();
    assert_eq!(result["total_matches"], 2000);
    assert_eq!(result["truncated"], true);
    assert_eq!(result["matches"][0]["element_index"], "g1:0");
    assert!(result["matches"].as_array().unwrap().len() <= 50);
    assert!(result.to_string().len() < 33_000);
}
