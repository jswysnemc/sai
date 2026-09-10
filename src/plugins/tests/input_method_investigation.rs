use super::services_support::{tool_call, ModelFixture, ModelReply};
use super::support::FixtureHost;
use crate::paths::SaiPaths;
use crate::plugins::discovery::{find, PluginDescriptor};
use crate::plugins::registry::register_descriptor;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

const PLUGIN: &str = "input-method-investigation";
const TOOL: &str = "linux_input_method_diagnose";
const REPORT: &str = "以下是诊断报告\n\n## 问题分析\n目标进程缺少输入法模块\n\n## 已确认事实\n读取到了目标进程环境\n\n## 推荐修复\n按证据调整启动环境";

/// 【输入法调查测试】【调查注册】加载真实 Lua 包，在其后注册证据工具以验证目录没有提前固定。
/// @param fixture 本地模型；paths 为临时路径；settings 为插件设置；change 可调整授权和资源限制
/// @returns 绑定实际选定模型及证据样本的工具表
fn registry(
    fixture: &ModelFixture,
    paths: &SaiPaths,
    settings: Value,
    change: impl FnOnce(&mut PluginDescriptor),
) -> ToolRegistry {
    let config = fixture.config("stale-model");
    let mut tools = ToolRegistry::new();
    tools.configure_plugin_model(&config, paths);
    tools.set_plugin_model_client(&fixture.client("selected-input-model", paths));
    let mut plugin = find(&config, paths, PLUGIN).unwrap();
    plugin.setting.settings = settings;
    plugin.refresh_compatibility(&config, &paths).unwrap();
    change(&mut plugin);
    register_descriptor(&mut tools, plugin, Arc::new(FixtureHost::default()), false).unwrap();
    register_evidence(&mut tools);
    tools
}

/// 【输入法调查测试】【证据样本】在真实注册表中提供可核对参数的原生诊断结果。
/// @param tools 当前工具表
/// @returns 无
fn register_evidence(tools: &mut ToolRegistry) {
    tools.register(ToolSpec::new(
        "check_issue",
        "Read input method evidence",
        json!({"type":"object"}),
        |args| async move {
            Ok(
                json!({"ok":true, "kind":"diagnostic_evidence", "arguments":args,
                "facts":{"input_method.target_env":{"GTK_IM_MODULE":"fcitx","LANG":"zh_CN.UTF-8"}},
                "checks":[{"id":"fixture.process","status":"ok","detail":"runtime observed"}]})
                .to_string(),
            )
        },
    ));
}

/// 【输入法调查测试】【模型建议】创建对输入法证据工具的正式函数调用。
/// @returns 一次模型响应
fn evidence_reply() -> ModelReply {
    ModelReply::delta(json!({"role":"assistant", "tool_calls":[
        tool_call(0, "check_issue", json!({"area":"input_method", "target":"fixture-app", "depth":"quick"}))
    ]}))
}

/// 【输入法调查测试】【实际执行】经工具入口执行并解析公开结果。
/// @param tools 当前工具表；arguments 为用户参数
/// @returns 最终报告和统计
async fn investigate(tools: &ToolRegistry, arguments: Value) -> Value {
    serde_json::from_str(&tools.call(TOOL, &arguments.to_string()).await.unwrap()).unwrap()
}

/// 【输入法调查测试】【完整链路】当前模型取得后来注册的证据工具，两个响应只累计 40 token。
#[tokio::test]
async fn lua_investigation_uses_the_current_model_and_late_registered_evidence() {
    let fixture = ModelFixture::start(vec![evidence_reply(), ModelReply::text(REPORT)]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let tools = registry(&fixture, &paths, json!({}), |_| {});
    let output = investigate(
        &tools,
        json!({"issue":"  无法输入中文  ","target":" fixture-app "}),
    )
    .await;
    assert_eq!(output["kind"], "linux_input_method_diagnosis");
    assert_eq!(output["issue"], "无法输入中文");
    assert_eq!(output["target"], "fixture-app");
    assert!(output["final_answer"]
        .as_str()
        .unwrap()
        .starts_with("## 问题分析\n"));
    assert!(output["output_instruction"]
        .as_str()
        .unwrap()
        .contains("final_answer"));
    assert_eq!(
        output["stats"],
        json!({"tool_calls":1,"tool_ok":1,"tool_errors":0,
        "prompt_tokens":20,"completion_tokens":20,"total_tokens":40,"token_estimate":40,
        "token_estimate_method":"provider_usage","token_estimate_is_actual":true})
    );
    let requests = fixture.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|request| request["model"] == "selected-input-model"));
    let transcript = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(transcript.contains("<tool_result name=\"check_issue\" ok=\"true\">"));
    assert!(transcript.contains("runtime observed"));
    assert!(transcript.contains("zh_CN.UTF-8"));
    assert!(requests
        .iter()
        .all(|request| request["messages"].as_array().unwrap().last().unwrap()["role"] == "user"));
}

/// 【输入法调查测试】【正式目录】应用共用工具表必须实际使用 Lua，并向调查公开诊断和知识库入口。
#[tokio::test]
async fn builtin_registry_exposes_lua_investigation_with_the_complete_current_catalog() {
    let fixture = ModelFixture::start(vec![evidence_reply(), ModelReply::text(REPORT)]).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = fixture.config("builtin-input-model");
    let mut tools = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert_eq!(tools.plugin_owner(TOOL), Some(PLUGIN));
    assert!(tools.plugin_diagnostics().is_empty());
    register_evidence(&mut tools);
    let output = investigate(&tools, json!({"issue":"无法输入中文"})).await;
    assert!(output["target"].is_null());
    let requests = fixture.requests();
    let names = requests[0]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    for name in [
        "check_issue",
        "fcitx5_input_method_wiki_qurey",
        "search_knowledge_base",
    ] {
        assert!(
            names.contains(&name),
            "missing late registered tool: {name}"
        );
    }
    assert!(!names.contains(&TOOL));
    assert!(!names.contains(&"linux_game_compatibility"));
    assert!(!names.contains(&"run_command"));
}

/// 【输入法调查测试】【预算收尾】业务、宿主工具和模型请求预算均能触发一次无工具最终报告。
#[tokio::test]
async fn each_budget_stops_excess_tools_and_reserves_the_final_report() {
    for boundary in ["business", "host-tools", "model"] {
        let fixture = ModelFixture::start(vec![
            ModelReply::delta(json!({"role":"assistant","tool_calls":[
                tool_call(0,"check_issue",json!({})),tool_call(1,"web_fetch",json!({}))
            ]})),
            ModelReply::text(REPORT),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut tools = registry(
            &fixture,
            &paths,
            json!({"max_tool_steps":if boundary == "business" {1} else {0}}),
            |plugin| {
                if boundary == "host-tools" {
                    plugin.package.manifest.limits.tool_calls = 1;
                }
                if boundary == "model" {
                    plugin.package.manifest.limits.model_requests = 2;
                }
            },
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        tools.register(ToolSpec::new(
            "web_fetch",
            "Count second probe",
            json!({"type":"object"}),
            move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                async { Ok("second probe".into()) }
            },
        ));
        let output = investigate(&tools, json!({"issue":"无法输入中文"})).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            usize::from(boundary == "model")
        );
        assert_eq!(output["stats"]["total_tokens"], 40);
        let requests = fixture.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1]
            .get("tools")
            .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
        assert!(
            requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("<tool_budget_reached>")
        );
    }
}

/// 【输入法调查测试】【执行回收】记录正在执行的原生探测 Future 何时结束。
struct ProbeRelease(Arc<AtomicBool>);

impl Drop for ProbeRelease {
    /// 【输入法调查测试】【执行回收】超时或取消时更新释放记录。
    /// @returns 无
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// 【输入法调查测试】【超时继续】原生探测超时后释放 Future，后续探测、统计、进度和最终报告继续完成。
#[tokio::test]
async fn timed_out_probe_is_released_and_diagnosis_continues() {
    for mode in ["summary", "full"] {
        let fixture = ModelFixture::start(vec![
            ModelReply::delta(json!({"role":"assistant","tool_calls":[
                tool_call(0,"check_issue",json!({"detail":"中文\"\n".repeat(1500)})),
                tool_call(1,"check_os_info",json!({}))
            ]})),
            ModelReply::text(REPORT),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut tools = registry(
            &fixture,
            &paths,
            json!({"progress_mode":mode,"tool_timeout_ms":15}),
            |_| {},
        );
        let released = Arc::new(AtomicBool::new(false));
        let observed = released.clone();
        tools.register(ToolSpec::new(
            "check_issue",
            "Blocked probe",
            json!({"type":"object"}),
            move |_| {
                let observed = observed.clone();
                async move {
                    let _release = ProbeRelease(observed);
                    std::future::pending::<()>().await;
                    Ok("unreachable".into())
                }
            },
        ));
        tools.register(ToolSpec::new(
            "check_os_info",
            "Next probe",
            json!({"type":"object"}),
            |_| async { Ok("next probe succeeded".into()) },
        ));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let output = tools
            .call_with_progress(TOOL, r#"{"issue":"无法输入中文"}"#, sender)
            .await
            .unwrap();
        let output: Value = serde_json::from_str(&output.content).unwrap();
        assert!(released.load(Ordering::SeqCst));
        assert_eq!(output["stats"]["tool_calls"], 2);
        assert_eq!(output["stats"]["tool_ok"], 1);
        assert_eq!(output["stats"]["tool_errors"], 1);
        let requests = fixture.requests();
        let transcript = requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap();
        assert!(transcript.contains("tool call timed out"));
        assert!(transcript.contains("next probe succeeded"));
        let mut saw_error = false;
        while let Ok(message) = receiver.try_recv() {
            assert!(message.len() <= 4096);
            if let Some(text) = message
                .strip_prefix("__subtool_call__")
                .or_else(|| message.strip_prefix("__subtool_result__"))
            {
                let value: Value = serde_json::from_str(text).unwrap();
                saw_error |= value.get("ok") == Some(&json!(false));
            } else {
                saw_error |= message.contains(" error");
            }
        }
        assert!(saw_error);
    }
}

/// 【输入法调查测试】【缺少用量】缺失或混合 usage 时保留估算标签，只累计供应商实际提供的用量。
#[tokio::test]
async fn missing_usage_and_optional_target_keep_the_public_contract() {
    for mixed in [false, true] {
        let fixture = ModelFixture::start(vec![
            evidence_reply().without_usage(),
            if mixed {
                ModelReply::text(REPORT)
            } else {
                ModelReply::text(REPORT).without_usage()
            },
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let tools = registry(&fixture, &paths, json!({"progress_mode":"hidden"}), |_| {});
        let output = investigate(&tools, json!({"issue":"无法输入中文","target":"  "})).await;
        assert!(output["target"].is_null());
        assert_eq!(output["stats"]["total_tokens"], if mixed { 20 } else { 0 });
        assert_eq!(output["stats"]["token_estimate_is_actual"], false);
        assert_eq!(
            output["stats"]["token_estimate_method"],
            if mixed {
                "provider_usage_plus_estimate"
            } else {
                "rough_char_estimate"
            }
        );
        assert!(output["stats"]["token_estimate"].as_u64().unwrap() > 20);
    }
}

/// 【输入法调查测试】【授权边界】撤销模型授权和非法问题在请求前失败，未授权工具不能由模型建议绕过。
#[tokio::test]
async fn model_revocation_invalid_input_and_tool_grants_are_enforced() {
    let fixture = ModelFixture::start(Vec::new()).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let tools = registry(&fixture, &paths, json!({}), |plugin| {
        let mut grants = plugin.grants();
        grants.model = false;
        plugin.setting.grants = Some(grants);
    });
    let error = tools
        .call(TOOL, r#"{"issue":"无法输入中文"}"#)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("model capability is not allowed"));
    for arguments in [json!({}), json!({"issue":"  "}), json!({"issue":false})] {
        assert!(tools.call(TOOL, &arguments.to_string()).await.is_err());
    }
    assert!(fixture.requests().is_empty());

    let fixture = ModelFixture::start(vec![evidence_reply(), ModelReply::text(REPORT)]).await;
    let tools = registry(&fixture, &paths, json!({}), |plugin| {
        let mut grants = plugin.grants();
        grants.tools.clear();
        plugin.setting.grants = Some(grants);
    });
    let output = investigate(&tools, json!({"issue":"无法输入中文"})).await;
    assert_eq!(output["stats"]["tool_errors"], 1);
    let requests = fixture.requests();
    assert!(requests[0]
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
    assert!(
        requests[1]["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("capability is not allowed")
    );
}

/// 【输入法调查测试】【长调查】超过进度额度后继续工作，并在消息上限前请求无工具报告。
#[tokio::test]
async fn long_diagnosis_respects_message_and_progress_limits() {
    let replies = (0..130).map(|_| ModelReply::delta(json!({"role":"assistant",
        "content":"## 问题分析\n整理现有证据","tool_calls":[tool_call(0,"check_issue",json!({}))]
    }))).collect();
    let fixture = ModelFixture::start(replies).await;
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let tools = registry(
        &fixture,
        &paths,
        json!({"max_tool_steps":0,"progress_mode":"full"}),
        |_| {},
    );
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let output = tools
        .call_with_progress(TOOL, r#"{"issue":"无法输入中文"}"#, sender)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(&output.content).unwrap();
    assert!(output["stats"]["tool_calls"].as_u64().unwrap() > 64);
    let requests = fixture.requests();
    assert!(requests.len() < 130);
    assert!(requests
        .iter()
        .all(|request| request["messages"].as_array().unwrap().len() <= 256));
    assert!(requests
        .last()
        .unwrap()
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
    let mut count = 0;
    while let Ok(message) = receiver.try_recv() {
        assert!(message.len() <= 4096);
        count += 1;
    }
    assert_eq!(count, 128);
}
