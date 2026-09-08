use super::support::{call_json, runtime, FixtureHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

const PLUGIN: &str = "linux-game-signals";
const TOOL: &str = "gather_linux_game_compatibility_signals";
const STEAM: &str = r#"{"items":[{"id":1091500,"name":"Cyberpunk 2077","tiny_image":"https://example.test/game.jpg"}]}"#;
const PLAYABLE: &str = "<p>Works</p><p>Recommended Proton</p><p>Proton 9.0-3</p><p>Steam Deck Verified</p><h2>Known issues</h2><p>部分模组需要调整</p><h2>Fixes</h2><p>Use Proton</p><h2>Verdict</h2>";

/// 【游戏信号测试】【完整采集】真实 Lua 包读取四个来源，保留请求契约与结果结构。
#[tokio::test]
async fn four_sources_produce_the_existing_evidence_contract() {
    let host = Arc::new(FixtureHost::new(&[
        (200, STEAM),
        (200, r#"{"tier":"gold","total":120}"#),
        (200, PLAYABLE),
        (200, "<p>Running Easy Anti-Cheat BattlEye</p>"),
    ]));
    let plugin = runtime(PLUGIN, host.clone());
    let result = call_json(&plugin, TOOL, json!({"game":"Linux 能玩赛博朋克 2077 吗"})).await;
    assert_eq!(result["search_query"], "Cyberpunk 2077");
    assert_eq!(result["query_candidates"], json!(["Cyberpunk 2077"]));
    assert_eq!(result["matched_name"], "Cyberpunk 2077");
    assert_eq!(result["verdict"]["traffic_light"], "green");
    assert_eq!(result["confidence"]["level"], "high");
    assert_eq!(result["needs_followup"], false);
    assert_eq!(result["confidence"]["followup_reason"], Value::Null);
    assert_eq!(
        result["can_i_play_on_linux"]["source_recommended_proton"],
        "Proton 9.0-3"
    );
    assert!(result["can_i_play_on_linux"]
        .get("recommended_proton")
        .is_none());
    assert_eq!(result["are_we_anticheat_yet"]["status"], "Running");
    assert_eq!(result["are_we_anticheat_yet"]["mentions_eac"], true);
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    let first = reqwest::Url::parse(&requests[0].url).unwrap();
    assert_eq!(first.host_str(), Some("store.steampowered.com"));
    assert_eq!(first.path(), "/api/storesearch/");
    assert_eq!(
        first.query_pairs().collect::<Vec<_>>(),
        vec![
            ("term".into(), "Cyberpunk 2077".into()),
            ("l".into(), "english".into()),
            ("cc".into(), "US".into()),
        ]
    );
    assert_eq!(
        requests[1].url,
        "https://www.protondb.com/api/v1/reports/summaries/1091500.json"
    );
    assert_eq!(
        requests[2].url,
        "https://caniplayonlinux.com/games/cyberpunk-2077/"
    );
    assert_eq!(
        requests[3].url,
        "https://areweanticheatyet.com/game/cyberpunk-2077"
    );
    for request in requests.iter() {
        assert_eq!(request.method, "GET");
        assert_eq!(request.timeout_ms, 20_000);
        assert_eq!(
            request.headers["user-agent"],
            "sai-linux-game-compatibility/0.1"
        );
        assert!(request.body.is_none());
    }
}

/// 【游戏信号测试】【候选回退】使用排序后的名称候选，首个页面失败后继续读取备用路径。
#[tokio::test]
async fn page_candidates_are_sorted_and_each_failure_is_recorded() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"items":[{"id":10,"name":"Alpha Name"}]}"#),
        (200, r#"{"tier":"silver"}"#),
        (404, "missing"),
        (200, "<p>Partial</p>"),
        (503, "unavailable"),
        (200, "<p>Supported</p>"),
    ]));
    let result = call_json(
        &runtime(PLUGIN, host.clone()),
        TOOL,
        json!({"game":"Zebra Name"}),
    )
    .await;
    for name in ["can_i_play_on_linux", "are_we_anticheat_yet"] {
        let attempts = result["source_attempts"][name].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["slug"], "alpha-name");
        assert_eq!(attempts[0]["ok"], false);
        assert!(!attempts[0]["error"].as_str().unwrap().is_empty());
        assert_eq!(attempts[1]["slug"], "zebra-name");
        assert_eq!(attempts[1]["ok"], true);
    }
    assert_eq!(result["verdict"]["traffic_light"], "yellow");
    assert_eq!(host.requests.lock().unwrap().len(), 6);
}

/// 【游戏信号测试】【部分来源】没有 Steam 匹配时跳过评级，仍保留其他来源与跟进建议。
#[tokio::test]
async fn missing_steam_does_not_discard_other_sources() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"items":[]}"#),
        (200, "<p>Works</p>"),
        (200, "<p>Running</p>"),
    ]));
    let result = call_json(&runtime(PLUGIN, host.clone()), TOOL, json!({"game":"原神"})).await;
    assert_eq!(result["steam"], Value::Null);
    assert_eq!(result["protondb"], Value::Null);
    assert_eq!(result["verdict"]["label"], "可玩");
    assert_eq!(result["confidence"]["level"], "medium");
    assert_eq!(result["needs_followup"], true);
    assert_eq!(
        result["confidence"]["source_coverage"]["steam_appid"],
        false
    );
    assert_eq!(result["sources"]["steam"], Value::Null);
    assert_eq!(host.requests.lock().unwrap().len(), 3);
}

/// 【游戏信号测试】【JSON 空值】成功取得的 null、false 和空页面仍计入来源覆盖。
#[tokio::test]
async fn null_and_false_payloads_are_distinct_from_failed_requests() {
    for rating in ["null", "false", "{}"] {
        let host = Arc::new(FixtureHost::new(&[
            (200, STEAM),
            (200, rating),
            (200, ""),
            (200, ""),
        ]));
        let result = call_json(
            &runtime(PLUGIN, host),
            TOOL,
            json!({"game":"Cyberpunk 2077"}),
        )
        .await;
        assert_eq!(
            result["protondb"],
            serde_json::from_str::<Value>(rating).unwrap()
        );
        assert_eq!(
            result["confidence"]["source_coverage"],
            json!({
                "steam_appid":true,"protondb":true,"can_i_play_on_linux":true,"are_we_anticheat_yet":true,
            })
        );
        assert_eq!(result["can_i_play_on_linux"]["text_excerpt"], "");
        assert_eq!(result["are_we_anticheat_yet"]["status"], Value::Null);
    }
}

/// 【游戏信号测试】【Steam 异常字段】首项 null 或 false 仍是匹配项，负数和浮点标识不能触发评级请求。
#[tokio::test]
async fn steam_item_shapes_preserve_matching_and_integer_id_rules() {
    for item in ["null", "false", r#"{"id":-1}"#, r#"{"id":1.0}"#] {
        let response = format!("{{\"items\":[{item}]}}");
        let host = Arc::new(FixtureHost::new(&[(200, &response), (200, ""), (200, "")]));
        let result = call_json(
            &runtime(PLUGIN, host.clone()),
            TOOL,
            json!({"game":"Test Game"}),
        )
        .await;
        assert!(result["steam"].is_object(), "{item}");
        assert_eq!(result["source_attempts"]["steam"][0]["ok"], true, "{item}");
        assert_eq!(result["matched_name"], "Test Game");
        assert_eq!(
            result["confidence"]["source_coverage"]["steam_appid"],
            false
        );
        assert_eq!(result["sources"]["protondb"], Value::Null);
        assert_eq!(host.requests.lock().unwrap().len(), 3);
    }
}

/// 【游戏信号测试】【零标识与空名称】零 App ID 有效，已匹配的空名称仍进入原路径候选。
#[tokio::test]
async fn zero_app_id_and_empty_matched_name_keep_their_original_meaning() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"items":[{"id":0,"name":"","tiny_image":false}]}"#),
        (200, "{}"),
        (200, ""),
        (200, ""),
    ]));
    let result = call_json(
        &runtime(PLUGIN, host.clone()),
        TOOL,
        json!({"game":"Test Game"}),
    )
    .await;
    assert_eq!(result["steam"]["url"], false);
    assert_eq!(result["matched_name"], "");
    assert_eq!(result["confidence"]["source_coverage"]["steam_appid"], true);
    assert_eq!(
        result["sources"]["steam"],
        "https://store.steampowered.com/app/0/"
    );
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[1].url,
        "https://www.protondb.com/api/v1/reports/summaries/0.json"
    );
    assert_eq!(requests[2].url, "https://caniplayonlinux.com/games//");
    assert_eq!(requests[3].url, "https://areweanticheatyet.com/game/");
}

/// 【游戏信号测试】【空路径候选】没有可用路径时不发送页面请求，尝试记录仍序列化为数组。
#[tokio::test]
async fn empty_slug_candidates_do_not_create_requests_or_object_shaped_arrays() {
    let host = Arc::new(FixtureHost::new(&[(200, r#"{"items":[]}"#)]));
    let result = call_json(&runtime(PLUGIN, host.clone()), TOOL, json!({"game":"---"})).await;
    assert_eq!(result["source_attempts"]["can_i_play_on_linux"], json!([]));
    assert_eq!(result["source_attempts"]["are_we_anticheat_yet"], json!([]));
    assert_eq!(result["sources"]["can_i_play_on_linux"], Value::Null);
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【游戏信号测试】【授权与错误恢复】撤销网络授权后没有请求抵达宿主，参数错误也不污染下一次调用。
#[tokio::test]
async fn revoked_grants_and_invalid_arguments_never_reach_the_host() {
    let package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == PLUGIN)
        .unwrap();
    let host = Arc::new(FixtureHost::default());
    let plugin =
        PluginRuntime::load(package, json!({}), Capabilities::default(), host.clone()).unwrap();
    assert!(plugin
        .call_tool(
            TOOL,
            json!({"game":"\u{3000}"}),
            InvocationContext::default()
        )
        .await
        .is_err());
    let result = call_json(&plugin, TOOL, json!({"game":"Rust Game"})).await;
    assert_eq!(result["confidence"]["level"], "low");
    assert_eq!(result["needs_followup"], true);
    assert!(host.requests.lock().unwrap().is_empty());
    for source in ["steam", "can_i_play_on_linux", "are_we_anticheat_yet"] {
        assert_eq!(result["source_attempts"][source][0]["ok"], false);
        assert!(result["source_attempts"][source][0]["error"]
            .as_str()
            .unwrap()
            .contains("not allowed"));
    }
}

/// 【游戏信号测试】【旧开关与显式设置】兼容旧启用开关，独立插件设置仍拥有最终优先级。
#[test]
fn plugin_settings_override_the_legacy_game_switch() {
    use crate::plugins::config::{save_config, PluginConfig, PluginSetting};
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = crate::config::AppConfig::default();
    config.plugins.linux_game_compatibility.enabled = false;
    assert!(!crate::tools::readonly_registry(&config, &paths).contains(TOOL));
    let mut settings = PluginConfig::default();
    settings.plugins.insert(
        PLUGIN.into(),
        PluginSetting {
            enabled: true,
            ..Default::default()
        },
    );
    save_config(&paths, &settings).unwrap();
    assert!(crate::tools::readonly_registry(&config, &paths).contains(TOOL));
    settings.plugins.get_mut(PLUGIN).unwrap().enabled = false;
    save_config(&paths, &settings).unwrap();
    config.plugins.linux_game_compatibility.enabled = true;
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(TOOL));
}
