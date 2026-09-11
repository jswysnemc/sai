use super::xuanxue_support::{assert_consumed, fixed_runtime, load, package};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};

/// 【玄学迁移测试】【参数拒绝】非法类型、非对象与额外字段在随机抽取前拒绝
/// @returns 无；原版宽容但违反 Schema 的输入统一拒绝，之后正常调用仍能成功
#[tokio::test]
async fn xuanxue_rejects_invalid_arguments_before_sampling_and_recovers() {
    let plugin = fixed_runtime(json!([[1, 6, 4]]));
    let mut rejected = 0;
    for source in [
        include_str!("fixtures/xuanxue_draws.json"),
        include_str!("fixtures/xuanxue_dice.json"),
    ] {
        let fixtures: Value = serde_json::from_str(source).unwrap();
        for case in fixtures["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["schema_valid"] == false)
        {
            let error = plugin
                .call_tool(
                    case["tool"].as_str().unwrap(),
                    serde_json::from_str(case["args_json"].as_str().unwrap()).unwrap(),
                    InvocationContext::default(),
                )
                .await
                .unwrap_err();
            assert!(
                format!("{error:#}").contains("arguments"),
                "{}: {error:#}",
                case["label"]
            );
            rejected += 1;
        }
    }
    assert_eq!(rejected, 26);
    let output = plugin
        .call_tool("roll_dice", json!({}), InvocationContext::default())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap()["rolls"],
        json!([4])
    );
    assert_consumed(&plugin, 1).await;
}

/// 【玄学迁移测试】【总和溢出】把原版 panic 或回绕改为明确错误，仍允许落在范围内的最大修正值
/// @returns 无；根据实际抽取总和判断溢出，每次错误后下一次调用可以继续
#[tokio::test]
async fn xuanxue_rejects_overflow_and_preserves_signed_boundaries() {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/xuanxue_dice.json")).unwrap();
    let cases = fixtures["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["expected"]["panic"] == true)
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let mut draws = case["draws"].as_array().unwrap().clone();
        draws.push(json!([1, 6, 1]));
        let plugin = fixed_runtime(json!(draws));
        let error = plugin
            .call_tool(
                "roll_dice",
                serde_json::from_str(case["args_json"].as_str().unwrap()).unwrap(),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("modified dice total exceeds signed 64-bit range"),
            "{error:#}"
        );
        let output = plugin
            .call_tool(
                "roll_dice",
                json!({"modifier":i64::MAX-1}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap()["modified_total"],
            json!(i64::MAX)
        );
        assert_consumed(&plugin, draws.len()).await;
    }
}

/// 【玄学迁移测试】【随机状态】相同测试种子的独立 VM 各自推进状态，克隆仍共享所属实例
/// @returns 无；业务使用 Lua 随机源，测试种子只存在于测试入口前缀
#[tokio::test]
async fn xuanxue_random_sequences_are_scoped_to_the_plugin_instance() {
    let first = load(package("math.randomseed(123, 456)", ""));
    let second = load(package("math.randomseed(123, 456)", ""));
    let cloned = first.clone();
    let context = InvocationContext::default();
    let args = json!({"count":100,"sides":1000});
    let initial = first
        .call_tool("roll_dice", args.clone(), context.clone())
        .await
        .unwrap();
    let advanced = cloned
        .call_tool("roll_dice", args.clone(), context.clone())
        .await
        .unwrap();
    assert_ne!(initial, advanced);
    assert_eq!(
        initial,
        second
            .call_tool("roll_dice", args.clone(), context.clone())
            .await
            .unwrap()
    );
    assert_eq!(
        advanced,
        second.call_tool("roll_dice", args, context).await.unwrap()
    );
}
