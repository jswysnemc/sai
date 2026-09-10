use super::support::FixtureHost;
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【哈希边界测试】【实际资源限制】发布包在输入、输出和累计计算超限后仍可处理正常请求
/// @returns 无；错误来自实际业务和宿主边界，下一次调用获得独立预算
#[tokio::test]
async fn bundled_hash_codec_enforces_limits_and_recovers() {
    let mut package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "hash-codec")
        .unwrap();
    package.manifest.limits.output_bytes = 1024;
    package.manifest.limits.instructions = 4000;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(FixtureHost::default()),
    )
    .unwrap();
    for (tool, args, expected_error) in [
        (
            "calculate_hash",
            json!({"input_text":"x".repeat(1025)}),
            "input exceeds plugin size limit",
        ),
        (
            "decode_encoded_text",
            json!({"input_text":"x".repeat(1025),"input_format":"rot13"}),
            "input exceeds plugin size limit",
        ),
        (
            "decode_encoded_text",
            json!({"input_text":"/".repeat(1024),"input_format":"base64"}),
            "encoding output exceeds plugin size limit",
        ),
        (
            "calculate_hash",
            json!({"input_text":"x".repeat(1024),"algorithms":"all"}),
            "instruction budget",
        ),
    ] {
        let error = plugin
            .call_tool(tool, args, InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected_error), "{error:#}");
        let result = plugin
            .call_tool(
                "calculate_hash",
                json!({"input_text":"abc"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["results"]["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
