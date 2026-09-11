use super::xuanxue_support::{load, package};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};

/// 【玄学迁移测试】【指令限制】在随机源注入额外计算，验证实际骰子调用和后续恢复
/// @returns 无；加载与一次简单抽取成功，累计随机计算触发指令限制
#[tokio::test]
async fn xuanxue_instruction_exhaustion_does_not_break_later_calls() {
    let mut package = package(
        r#"
        local original = math.random
        --- 【玄学迁移测试】【预算替身】每次抽取追加有界整数运算，保持随机结果不变
        --- @param low integer 下界
        --- @param high integer 上界
        --- @return integer 原随机值
        math.random = function(low, high)
            local value = original(low, high)
            for index = 1, 32 do value = value | 0 end
            return value
        end
    "#,
        "",
    );
    package.manifest.limits.instructions = 1000;
    let plugin = load(package);
    let error = plugin
        .call_tool(
            "roll_dice",
            json!({"count":100,"sides":1000}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("instruction budget"),
        "{error:#}"
    );
    let output = plugin
        .call_tool("roll_dice", json!({}), InvocationContext::default())
        .await
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&output).unwrap()["count"], 1);
}

/// 【玄学迁移测试】【超时恢复】替换第一次随机抽取为耗时计算，验证真实回调时限和后续恢复
/// @returns 无；超时不会永久占用 VM，也不会取消后续正常请求
#[tokio::test]
async fn xuanxue_timeout_releases_the_vm_for_the_next_call() {
    let mut package = package(
        r#"
        local original = math.random
        local first = true
        --- 【玄学迁移测试】【超时替身】第一次抽取耗尽时限，后续使用正常随机源
        --- @param low integer 下界
        --- @param high integer 上界
        --- @return integer 正常调用的随机值
        math.random = function(low, high)
            if first then
                first = false
                while true do string.rep("x", 65536):find("y", 1, true) end
            end
            return original(low, high)
        end
    "#,
        "",
    );
    package.manifest.limits.instructions = 20_000_000;
    package.manifest.limits.timeout_ms = 100;
    let plugin = load(package);
    let error = plugin
        .call_tool(
            "draw_zhouyi_hexagram",
            json!({}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    assert!(!plugin
        .call_tool(
            "draw_zhouyi_hexagram",
            json!({}),
            InvocationContext::default()
        )
        .await
        .unwrap()
        .is_empty());
}

/// 【玄学迁移测试】【发布限额】发布包的正常最大请求在最低合法输出预算内完成
/// @returns 无；输出保持 100 个整数点数和精确总和，结果不会接近发布输出上限
#[tokio::test]
async fn xuanxue_maximum_roll_fits_the_declared_resource_limits() {
    let mut package = package("", "");
    package.manifest.limits.output_bytes = 1024;
    let plugin = load(package);
    let output = plugin
        .call_tool(
            "roll_dice",
            json!({"count":u64::MAX,"sides":u64::MAX,"modifier":i64::MIN}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert!(output.len() < 1024);
    let value: Value = serde_json::from_str(&output).unwrap();
    let rolls = value["rolls"].as_array().unwrap();
    assert_eq!(rolls.len(), 100);
    let total: i64 = rolls
        .iter()
        .map(|roll| {
            let roll = roll.as_i64().unwrap();
            assert!((1..=1000).contains(&roll));
            roll
        })
        .sum();
    assert_eq!(value["total"], total);
    assert_eq!(value["modified_total"], i64::MIN + total);
}
