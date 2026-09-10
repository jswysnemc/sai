use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【通知对照测试】【Unicode 正文】使用原 Rust 函数生成的固定结果验证边界，不依赖 Git 历史。
#[tokio::test]
async fn notification_text_matches_48_frozen_native_reference_cases() {
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/notification_reference.json")).unwrap();
    assert_eq!(
        fixtures["source_commit"],
        "8a837a572e7e4d8021cf1931428c85856b172762"
    );
    let original = super::query_support::package(super::notification_policy::ID);
    let mut sources = original.sources().clone();
    sources.insert("init.lua".into(), r#"
        sai.register_tool({name='reference',description='Compare notification text',parameters={type='object'},
            execute=function(args) return require('policy').format_body(args.text, args.max_chars) end})
    "#.into());
    let runtime = PluginRuntime::load(
        PluginPackage::new(original.manifest, sources).unwrap(),
        json!({}),
        Capabilities::default(),
        Arc::new(super::support::FixtureHost::default()),
    )
    .unwrap();
    let cases = fixtures["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 48);
    for case in cases {
        let result = runtime
            .call_tool(
                "reference",
                json!({"text":case["text"],"max_chars":case["max_chars"]}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(result, case["expected"].as_str().unwrap(), "{case}");
    }
}
