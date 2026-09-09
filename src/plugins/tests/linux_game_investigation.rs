use super::services_support::{tool_call, ModelFixture, ModelReply};
use super::support::FixtureHost;
use crate::paths::SaiPaths;
use crate::plugins::discovery::{find, PluginDescriptor};
use crate::plugins::registry::register_descriptor;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const PLUGIN: &str = "linux-game-investigation";
const TOOL: &str = "linux_game_compatibility";
const SIGNALS: &str = "gather_linux_game_compatibility_signals";
const REPORT: &str = "以下是最终报告\n\n## 调查结果\n绿灯，可以游玩\n\n## 怎么玩\n启用 Proton 9.0-3\n\n## 注意事项\n部分模组需要调整";

/// 【游戏调查测试】【采集宿主】提供四来源证据，HTTP 调用仍由真实采集 Lua 包构造。
/// @returns 固定响应和可观察请求记录
fn signal_host() -> Arc<FixtureHost> {
    Arc::new(FixtureHost::new(&[
        (
            200,
            r#"{"items":[{"id":1091500,"name":"Cyberpunk 2077","tiny_image":"https://example.test/game.jpg"}]}"#,
        ),
        (200, r#"{"tier":"gold","total":120}"#),
        (
            200,
            "<p>Works</p><p>Recommended Proton</p><p>Proton 9.0-3</p>",
        ),
        (200, "<p>Running Easy Anti-Cheat BattlEye</p>"),
    ]))
}

/// 【游戏调查测试】【正式注册】加载调查包和采集包，并使用真实应用模型服务。
/// @param fixture 本地模型；paths 为临时目录；settings 为显式插件设置；change 可调整调查清单或授权
/// @returns 工具表和采集 HTTP 宿主
fn registry(
    fixture: &ModelFixture,
    paths: &SaiPaths,
    settings: Value,
    change: impl FnOnce(&mut PluginDescriptor),
) -> (ToolRegistry, Arc<FixtureHost>) {
    let config = fixture.config("game-model");
    let mut tools = ToolRegistry::new();
    tools.set_plugin_model_client(&fixture.client("game-model", paths));
    let host = signal_host();
    let mut investigation = find(&config, paths, PLUGIN).unwrap();
    investigation.setting.settings = settings;
    investigation.refresh_compatibility(&config).unwrap();
    change(&mut investigation);
    register_descriptor(&mut tools, investigation, host.clone(), false).unwrap();
    register_descriptor(
        &mut tools,
        find(&config, paths, "linux-game-signals").unwrap(),
        host.clone(),
        false,
    )
    .unwrap();
    (tools, host)
}

/// 【游戏调查测试】【调查请求】构造要求使用实际采集包的模型响应。
/// @returns 一次采集工具建议
fn gather_reply() -> ModelReply {
    ModelReply::delta(json!({"role":"assistant", "tool_calls":[
        tool_call(0, SIGNALS, json!({"game":"Cyberpunk 2077"}))
    ]}))
}

/// 【游戏调查测试】【调查执行】走正式工具入口并解析公开结果。
/// @param tools 当前注册表
/// @returns 最终报告和统计
async fn investigate(tools: &ToolRegistry) -> Value {
    serde_json::from_str(
        &tools
            .call(
                TOOL,
                r#"{"game":"  Cyberpunk 2077  ","issue":"multiplayer"}"#,
            )
            .await
            .unwrap(),
    )
    .unwrap()
}

/// 【游戏调查测试】【完整链路】真实调查 Lua、采集 Lua、HTTP 样本和两次模型请求累计 40 token，末次响应不能重复计费。
#[tokio::test]
async fn real_lua_investigation_uses_signal_plugin_and_counts_each_response_once() {
    let fixture = ModelFixture::start(vec![gather_reply(), ModelReply::text(REPORT)]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (tools, host) = registry(&fixture, &paths, json!({}), |_| {});
    assert_eq!(tools.plugin_owner(TOOL), Some(PLUGIN));
    assert_eq!(tools.plugin_owner(SIGNALS), Some("linux-game-signals"));
    let output = investigate(&tools).await;
    assert_eq!(output["ok"], true);
    assert_eq!(output["kind"], TOOL);
    assert_eq!(output["game_query"], "Cyberpunk 2077");
    assert_eq!(
        output["final_report"],
        REPORT
            .split_once("## 调查结果")
            .map(|(_, rest)| format!("## 调查结果{rest}"))
            .unwrap()
    );
    assert_eq!(
        output["stats"],
        json!({"tool_calls":1, "tool_ok":1, "tool_errors":0,
        "prompt_tokens":20, "completion_tokens":20, "total_tokens":40, "token_estimate":40,
        "token_estimate_method":"provider_usage", "token_estimate_is_actual":true})
    );
    assert!(output["output_instruction"]
        .as_str()
        .unwrap()
        .contains("final_report"));
    assert_eq!(host.requests.lock().unwrap().len(), 4);
    let requests = fixture.requests();
    assert_eq!(requests.len(), 2);
    let transcript = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(transcript.contains(&format!("<tool_result name=\"{SIGNALS}\" ok=\"true\">")));
    assert!(transcript.contains("\"traffic_light\": \"green\""));
    assert!(transcript.contains("Proton 9.0-3"));
    assert!(requests
        .iter()
        .all(|request| request["messages"].as_array().unwrap().last().unwrap()["role"] == "user"));
}

/// 【游戏调查测试】【单轮预算】同一响应中的超额工具只记为跳过，下一次模型请求不再提供工具。
#[tokio::test]
async fn multi_tool_response_skips_excess_calls_and_finalizes_without_tools() {
    let fixture = ModelFixture::start(vec![
        ModelReply::delta(json!({"role":"assistant", "tool_calls":[
            tool_call(0, SIGNALS, json!({"game":"Cyberpunk 2077"})),
            tool_call(1, "web_fetch", json!({"url":"https://example.test"})),
            tool_call(2, "web_fetch", json!({"url":"https://example.test/again"}))
        ]})),
        ModelReply::text(REPORT),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (mut tools, _) = registry(&fixture, &paths, json!({"max_tool_steps":1}), |_| {});
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    tools.register(ToolSpec::new(
        "web_fetch",
        "Count excess requests",
        json!({"type":"object"}),
        move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("unexpected".into()) }
        },
    ));
    let result = investigate(&tools).await;
    assert_eq!(result["stats"]["tool_calls"], 1);
    assert_eq!(result["stats"]["total_tokens"], 40);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let requests = fixture.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
    let messages = requests[1]["messages"].as_array().unwrap();
    assert_eq!(
        messages[messages.len() - 2]["content"]
            .as_str()
            .unwrap()
            .matches("tool skipped")
            .count(),
        2
    );
    assert!(messages.last().unwrap()["content"]
        .as_str()
        .unwrap()
        .contains("<tool_budget_reached>"));
}

/// 【游戏调查测试】【模型预算】业务未设置工具上限时，仍为最后一次无工具模型请求预留额度。
#[tokio::test]
async fn model_budget_reserves_a_final_report_request() {
    let fixture = ModelFixture::start(vec![gather_reply(), ModelReply::text(REPORT)]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (tools, _) = registry(&fixture, &paths, json!({"max_tool_steps":0}), |plugin| {
        plugin.package.manifest.limits.model_requests = 2;
    });
    let result = investigate(&tools).await;
    assert_eq!(result["stats"]["total_tokens"], 40);
    let requests = fixture.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
}

/// 【游戏调查测试】【上下文收尾】长调查在消息数量上限前请求最终报告，进度额度耗尽也不会中断业务。
#[tokio::test]
async fn long_investigations_finalize_before_message_and_progress_limits() {
    let replies = (0..130)
        .map(|_| {
            ModelReply::delta(json!({"role":"assistant",
                "content":"## 调查结果\n根据已有证据整理报告",
                "tool_calls":[tool_call(0, "check_os_info", json!({}))]
            }))
        })
        .collect();
    let fixture = ModelFixture::start(replies).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (mut tools, _) = registry(
        &fixture,
        &paths,
        json!({"progress_mode":"full", "max_tool_steps":0}),
        |_| {},
    );
    tools.register(ToolSpec::new(
        "check_os_info",
        "Read fixture OS",
        json!({"type":"object"}),
        |_| async { Ok("Linux fixture".into()) },
    ));
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let output = tools
        .call_with_progress(TOOL, r#"{"game":"Cyberpunk 2077"}"#, sender)
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(result["ok"], true);
    assert!(result["stats"]["tool_calls"].as_u64().unwrap() > 64);
    let requests = fixture.requests();
    assert!(requests.len() < 130);
    assert!(
        requests.last().unwrap()["messages"]
            .as_array()
            .unwrap()
            .len()
            >= 254
    );
    assert!(requests
        .iter()
        .all(|request| request["messages"].as_array().unwrap().len() <= 256));
    assert!(requests
        .last()
        .unwrap()
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
    let mut progress_count = 0;
    while let Ok(message) = receiver.try_recv() {
        assert!(message.len() <= 4096);
        progress_count += 1;
    }
    assert_eq!(progress_count, 128);
}

/// 【游戏调查测试】【用量缺失】纯估算和混合用量保持原公开标签，供应商统计只累计实际返回的响应。
#[tokio::test]
async fn absent_and_mixed_usage_are_reported_as_estimates() {
    for (first_missing, last_missing) in [(true, true), (true, false), (false, true)] {
        let first = if first_missing {
            gather_reply().without_usage()
        } else {
            gather_reply()
        };
        let last = if last_missing {
            ModelReply::text(REPORT).without_usage()
        } else {
            ModelReply::text(REPORT)
        };
        let fixture = ModelFixture::start(vec![first, last]).await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let (tools, _) = registry(&fixture, &paths, json!({}), |_| {});
        let output = investigate(&tools).await;
        let stats = &output["stats"];
        let actual = if first_missing && last_missing { 0 } else { 20 };
        assert_eq!(stats["total_tokens"], actual);
        assert_eq!(stats["prompt_tokens"], actual / 2);
        assert_eq!(stats["completion_tokens"], actual / 2);
        assert_eq!(stats["token_estimate_is_actual"], false);
        assert_eq!(
            stats["token_estimate_method"],
            if actual == 0 {
                "rough_char_estimate"
            } else {
                "provider_usage_plus_estimate"
            }
        );
        assert!(stats["token_estimate"].as_u64().unwrap() > actual as u64);
    }
}

/// 【游戏调查测试】【授权撤销】采集工具不可见或模型权限被撤销时，在任何模型请求和 HTTP 请求之前失败。
#[tokio::test]
async fn missing_signals_and_revoked_capabilities_fail_before_model_or_http_requests() {
    let fixture = ModelFixture::start(Vec::new()).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    for scenario in ["disabled-signals", "no-model", "no-tools"] {
        let (mut tools, host) = registry(&fixture, &paths, json!({}), |plugin| {
            let mut grants = plugin.grants();
            if scenario == "no-model" {
                grants.model = false;
            }
            if scenario == "no-tools" {
                grants.tools.clear();
            }
            plugin.setting.grants = Some(grants);
        });
        if scenario == "disabled-signals" {
            tools = tools.clone_excluding(&[SIGNALS]);
        }
        let error = tools
            .call(TOOL, r#"{"game":"Cyberpunk 2077"}"#)
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains(if scenario == "no-model" {
                "model capability is not allowed"
            } else {
                "linux-game-signals plugin is disabled"
            }),
            "{message}"
        );
        assert!(host.requests.lock().unwrap().is_empty());
        assert!(fixture.requests().is_empty());
    }
}

/// 【游戏调查测试】【失败进度】工具错误进入统计和后续模型上下文，概要显示 error，详细模式保持有界 JSON。
#[tokio::test]
async fn tool_failures_are_visible_in_statistics_transcripts_and_bounded_progress() {
    for mode in ["summary", "full"] {
        let fixture = ModelFixture::start(vec![ModelReply::delta(json!({"role":"assistant", "tool_calls":[
            tool_call(0, SIGNALS, json!({"game":"Cyberpunk 2077"})),
            tool_call(1, "web_fetch", json!({"url":"https://example.test", "detail":"中文\"\n".repeat(1500)}))
        ]})), ModelReply::text(REPORT)]).await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let (mut tools, _) = registry(
            &fixture,
            &paths,
            json!({"progress_mode":mode, "language":"zh"}),
            |_| {},
        );
        tools.register(ToolSpec::new(
            "web_fetch",
            "Fail query",
            json!({"type":"object"}),
            |_| async { anyhow::bail!("fixture fetch rejected") },
        ));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let output = tools
            .call_with_progress(TOOL, r#"{"game":"Cyberpunk 2077"}"#, sender)
            .await
            .unwrap();
        let result: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(result["stats"]["tool_calls"], 2);
        assert_eq!(result["stats"]["tool_ok"], 1);
        assert_eq!(result["stats"]["tool_errors"], 1);
        let mut messages = Vec::new();
        while let Ok(message) = receiver.try_recv() {
            messages.push(message);
        }
        assert!(messages.iter().all(|message| message.len() <= 4096));
        if mode == "summary" {
            assert!(messages
                .iter()
                .any(|message| message.contains("#2") && message.ends_with("error")));
        } else {
            let results = messages
                .iter()
                .filter_map(|message| message.strip_prefix("__subtool_result__"))
                .map(|text| serde_json::from_str::<Value>(text).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(results.len(), 2);
            assert_eq!(results[1]["ok"], false);
            for message in messages
                .iter()
                .filter_map(|message| message.strip_prefix("__subtool_call__"))
            {
                serde_json::from_str::<Value>(message).unwrap();
            }
        }
        let requests = fixture.requests();
        assert!(
            requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("fixture fetch rejected")
        );
    }
}
