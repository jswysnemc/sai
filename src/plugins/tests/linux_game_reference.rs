use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【游戏信号测试】【原版探针】在真实包中增加测试工具，以调用原业务模块进行独立对照。
/// @param host 不允许发生未声明请求的测试宿主
/// @returns 使用发布源码和清单的独立运行时，探针只在当前测试中注册
fn reference_runtime(host: Arc<FixtureHost>) -> PluginRuntime {
    let original = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "linux-game-signals")
        .unwrap();
    let mut sources = original.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(
        r#"
local query_rules = require("query")
local playable_rules = require("providers.can_i_play")
local anticheat_rules = require("providers.anticheat")
local verdict_rules = require("verdict")
local confidence_rules = require("confidence")

--- 【游戏信号测试】【原版探针】读取固定样本并调用发布包内的规则
--- @param input table 对照类型和原始输入
--- @return table 实际业务模块结果
local function reference(input)
    if input.kind == "query" then
        local candidates = query_rules.candidates(input.game)
        return {
            normalized = query_rules.normalize(input.game), slug = query_rules.slug(input.game),
            candidates = candidates, slugs = query_rules.slugs(candidates, "", false),
        }
    end
    if input.kind == "can_i_play" then return playable_rules.summary(input.html) end
    if input.kind == "anticheat" then return anticheat_rules.summary(input.html) end
    assert(input.kind == "analysis", "unknown reference case")
    local rating
    if input.protondb_present then rating = input.protondb end
    local playable = type(input.can_i_play) == "string" and input.can_i_play or nil
    local anti = type(input.anticheat) == "string" and input.anticheat or nil
    local app_id = input.appid
    if math.type(app_id) ~= "integer" or app_id < 0 then app_id = nil end
    local verdict = verdict_rules.evaluate(rating, playable, anti, input.issue)
    return {
        verdict = verdict,
        confidence = confidence_rules.evaluate(app_id, rating, playable, anti, verdict),
    }
end

sai.register_tool({name="reference", description="Check captured original results",
    parameters={type="object"}, execute=reference})
"#,
    );
    let grants = original.manifest.capabilities.clone();
    let package = PluginPackage::new(original.manifest, sources).unwrap();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【游戏信号测试】【原版对照】逐字段核对名称、页面摘要、判定和置信度，覆盖原有边界规则。
#[tokio::test]
async fn lua_rules_match_captured_original_rust_results() {
    let host = Arc::new(FixtureHost::default());
    let runtime = reference_runtime(host.clone());
    let mut checked = 0;
    for source in [
        include_str!("fixtures/linux-game/query.json"),
        include_str!("fixtures/linux-game/can_i_play.json"),
        include_str!("fixtures/linux-game/anticheat.json"),
        include_str!("fixtures/linux-game/analysis.json"),
    ] {
        let fixture: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            fixture["source_commit"],
            "b407d955ad84d4f55f0c0d93d1c05bccdeb7ecd8"
        );
        for case in fixture["cases"].as_array().unwrap() {
            let actual = runtime
                .call_tool(
                    "reference",
                    case["input"].clone(),
                    InvocationContext::default(),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&actual).unwrap(),
                case["output"],
                "{}",
                case["name"]
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 94);
    assert!(host.requests.lock().unwrap().is_empty());
}
