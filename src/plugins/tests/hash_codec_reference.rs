use super::example_support::runtime;
use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, ToolAccess};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【哈希迁移测试】【原版对照】固定旧版结果校验算法、别名、编码、Unicode 与完整 JSON 文本
/// @returns 无；错误信息必须保留原始原因，成功结果必须逐字节一致
#[tokio::test]
async fn hash_codec_matches_frozen_native_results() {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/hash_codec_reference.json")).unwrap();
    assert_eq!(fixtures["source_commit"], "f4b0bec");
    let host = Arc::new(FixtureHost::default());
    let plugin = runtime("hash-codec", host.clone());
    let cases = fixtures["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 175);
    for (index, case) in cases.iter().enumerate() {
        let result = plugin
            .call_tool(
                case["tool"].as_str().unwrap(),
                case["args"].clone(),
                InvocationContext::default(),
            )
            .await;
        if let Some(expected) = case["expected"]["error"].as_str() {
            let error = result.unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "case {index}: {case}\n{error:#}"
            );
        } else {
            assert_eq!(
                result.unwrap_or_else(|error| panic!("case {index}: {case}\n{error:#}")),
                case["expected"]["output"].as_str().unwrap(),
                "case {index}: {}",
                case["args"]
            );
        }
    }
    assert!(host.requests.lock().unwrap().is_empty());
    assert_eq!(plugin.tools().len(), 2);
    for definition in fixtures["definitions"].as_array().unwrap() {
        let actual = plugin
            .tools()
            .iter()
            .find(|tool| tool.name == definition["name"].as_str().unwrap())
            .unwrap();
        assert_eq!(actual.parameters, definition["parameters"]);
        assert_eq!(actual.access, ToolAccess::ReadOnly);
    }
}

/// 【哈希迁移测试】【参数契约】缺失字段、错误类型、额外字段及未知格式均在回调前失败
/// @returns 无；拒绝非法输入后仍能正常计算默认 SHA-256
#[tokio::test]
async fn hash_codec_validates_arguments_and_recovers_from_failures() {
    let plugin = runtime("hash-codec", Arc::new(FixtureHost::default()));
    for (tool, args) in [
        ("calculate_hash", json!({})),
        ("calculate_hash", json!({"input_text":42})),
        (
            "calculate_hash",
            json!({"input_text":"abc","algorithms":[]}),
        ),
        (
            "calculate_hash",
            json!({"input_text":"abc","input_format":"url"}),
        ),
        ("calculate_hash", json!({"input_text":"abc","extra":true})),
        ("decode_encoded_text", json!({"input_text":"abc"})),
        (
            "decode_encoded_text",
            json!({"input_text":"abc","input_format":null}),
        ),
        (
            "decode_encoded_text",
            json!({"input_text":"abc","input_format":"text"}),
        ),
        (
            "decode_encoded_text",
            json!({"input_text":"YWJj","input_format":"base64","text_encoding":false}),
        ),
    ] {
        let error = plugin
            .call_tool(tool, args.clone(), InvocationContext::default())
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(&format!("plugin tool {tool} arguments:")),
            "{args}: {error:#}"
        );
    }
    let output = plugin
        .call_tool(
            "calculate_hash",
            json!({"input_text":"abc"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"success":true,"byte_length":3,"results":{"sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}})
    );
}
