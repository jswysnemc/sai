use super::query_support::{package, QueryHost};
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【查询对照测试】【文本样本】展开重复文本表示，避免在固定样本中存储大量相同正文。
/// @param value 原始字符串或包含 repeat、count 及可选前后缀的对象
/// @returns 用于调用真实业务函数的完整文本
fn expand(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    format!(
        "{}{}{}",
        value["prefix"].as_str().unwrap_or(""),
        value["repeat"]
            .as_str()
            .unwrap()
            .repeat(value["count"].as_u64().unwrap() as usize),
        value["suffix"].as_str().unwrap_or("")
    )
}

/// 【查询对照测试】【真实模块】仅追加纯函数测试入口，正式工具继续使用发布源码中的查询函数。
/// @param input 固定输入及可选设置；host 为固定响应宿主
/// @returns 具有原工具和对照入口的独立运行时
fn runtime(input: &Value, host: Arc<QueryHost>) -> PluginRuntime {
    let original = package(input["plugin"].as_str().unwrap());
    let mut sources = original.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(
        r#"
--- 【查询对照测试】【纯函数入口】按固定样本调用正式模块中的币种、站点和截断函数
--- @param input table 固定输入
--- @return string|table 原业务函数返回值
local function reference(input)
    if input.op == "currency" then return require("currencies").code(input.text) end
    if input.op == "site" then return require("sites").select(input.text) end
    if input.op == "clip" then return require("page").clip(input.text) end
    error("unknown query reference operation")
end
sai.register_tool({name="reference",description="Compare query functions",access="read_only",
    parameters={type="object"},execute=reference})
"#,
    );
    let grants = original.manifest.capabilities.clone();
    PluginRuntime::load(
        PluginPackage::new(original.manifest, sources).unwrap(),
        input.get("settings").cloned().unwrap_or_else(|| json!({})),
        grants,
        host,
    )
    .unwrap()
}

/// 【查询对照测试】【原版行为】固定 Rust 结果验证 Unicode、JSON 数值、查询编排和实际请求顺序。
#[tokio::test]
async fn query_plugins_match_frozen_native_results_and_requests() {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/query_reference.json")).unwrap();
    assert_eq!(
        fixtures["source_commit"],
        "74ce52d7068ec1feae4cae352750a1404bb6d1d6"
    );
    let cases = fixtures["cases"].as_array().unwrap();
    for case in cases {
        let input = &case["input"];
        let expected = &case["expected"];
        let responses = input["responses"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let bodies = responses
            .iter()
            .map(|response| response.get("body").map(expand).unwrap_or_default())
            .collect::<Vec<_>>();
        let responses = responses
            .iter()
            .zip(&bodies)
            .map(|(response, body)| {
                if let Some(error) = response["error"].as_str() {
                    Err(error)
                } else {
                    Ok((response["status"].as_u64().unwrap() as u16, body.as_str()))
                }
            })
            .collect::<Vec<_>>();
        let host = Arc::new(QueryHost::new(&responses));
        let plugin = runtime(input, host.clone());
        let (tool, args) = if input["op"] == "tool" {
            let tool = match input["plugin"].as_str().unwrap() {
                "weather" => "get_weather",
                "exchange-rate" => "get_exchange_rate",
                "moegirl" => "query_moegirl",
                _ => unreachable!(),
            };
            (tool, input["args"].clone())
        } else {
            let mut args = input.clone();
            if input["op"] == "clip" {
                args["text"] = json!(expand(&input["text"]));
            }
            ("reference", args)
        };
        let output = plugin
            .call_tool(tool, args, InvocationContext::default())
            .await;
        if let Some(error) = expected["error"].as_str() {
            let error = error.replace("fixture HTTP status ", "HTTP status ");
            assert!(
                format!("{:#}", output.unwrap_err()).contains(&error),
                "{}: {error}",
                case["name"]
            );
        } else {
            let output = output.unwrap_or_else(|error| panic!("{}: {error:#}", case["name"]));
            if let Some(expected_json) = expected.get("output_json") {
                assert_eq!(
                    serde_json::from_str::<Value>(&output).unwrap(),
                    *expected_json,
                    "{}",
                    case["name"]
                );
            } else if let Some(hash) = expected["output_blake3"].as_str() {
                assert_eq!(json!(output.len()), expected["output_bytes"]);
                assert_eq!(
                    blake3::hash(output.as_bytes()).to_hex().as_str(),
                    hash,
                    "{}",
                    case["name"]
                );
            } else {
                assert_eq!(json!(output), expected["output"], "{}", case["name"]);
            }
        }
        let requests = host.requests.lock().unwrap();
        let urls = requests
            .iter()
            .map(|request| &request.url)
            .collect::<Vec<_>>();
        assert_eq!(json!(urls), expected["requests"], "{}", case["name"]);
    }
    assert_eq!(cases.len(), 99);
}
