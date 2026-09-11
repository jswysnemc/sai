use super::xuanxue_support::{assert_consumed, fixed_runtime, load, package};
use sai_plugin_runtime::{InvocationContext, ToolAccess};
use serde_json::Value;
use std::collections::BTreeSet;

/// 【玄学迁移测试】【原版对照】完整覆盖卦库、正逆塔罗、签文及骰子整数边界
/// @returns 无；合法且未溢出的原版结果必须逐字节相同，随机区间与次数一致
#[tokio::test]
async fn xuanxue_matches_all_frozen_native_results() {
    let mut compared = 0;
    let mut outcomes = BTreeSet::new();
    for source in [
        include_str!("fixtures/xuanxue_draws.json"),
        include_str!("fixtures/xuanxue_dice.json"),
    ] {
        let fixtures: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            fixtures["source_commit"],
            "5cbaad05acedfb76f18ab659a4064f3b1cb7e201"
        );
        let cases = fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["schema_valid"] == true && case["expected"]["output"].is_string())
            .collect::<Vec<_>>();
        for case in cases {
            let plugin = fixed_runtime(case["draws"].clone());
            let output = plugin
                .call_tool(
                    case["tool"].as_str().unwrap(),
                    serde_json::from_str(case["args_json"].as_str().unwrap()).unwrap(),
                    InvocationContext::default(),
                )
                .await
                .unwrap_or_else(|error| panic!("{}: {error:#}", case["label"]));
            assert_eq!(
                output,
                case["expected"]["output"].as_str().unwrap(),
                "{}",
                case["label"]
            );
            if case["tool"] != "roll_dice" {
                assert!(outcomes.insert(output));
            }
            assert_consumed(&plugin, case["draws"].as_array().unwrap().len()).await;
            compared += 1;
        }
    }
    assert_eq!(outcomes.len(), 64 + 78 * 2 + 7);
    assert_eq!(compared, 301);
}

/// 【玄学迁移测试】【参数契约】四个只读工具保留原 JSON Schema，不增加种子或随机控制参数
/// @returns 无；包不注册用户命令或事件，数值边界继续由业务实现默认值与收窄
#[test]
fn xuanxue_preserves_tool_schemas_and_readonly_access() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/xuanxue_dice.json")).unwrap();
    let plugin = load(package("", ""));
    assert_eq!(plugin.tools().len(), 4);
    assert!(plugin.commands().is_empty());
    assert!(plugin.events().is_empty());
    for definition in fixtures["definitions"].as_array().unwrap() {
        let tool = plugin
            .tools()
            .iter()
            .find(|tool| tool.name == definition["name"].as_str().unwrap())
            .unwrap();
        assert_eq!(tool.parameters, definition["parameters"]);
        assert_eq!(tool.access, ToolAccess::ReadOnly);
    }
}
