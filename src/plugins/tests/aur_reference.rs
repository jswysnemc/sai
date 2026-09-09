use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【AUR 测试】【原版对照】用原 Rust 生成的固定样本核对全部风险模式与文件选择规则。
#[tokio::test]
async fn aur_rules_match_frozen_native_reference_cases() {
    let risks: Value =
        serde_json::from_str(include_str!("fixtures/aur_risk_reference.json")).unwrap();
    let selections: Value =
        serde_json::from_str(include_str!("fixtures/aur_file_reference.json")).unwrap();
    let package = super::query_support::package("package-advisor");
    let mut sources = package.sources().clone();
    sources.insert("init.lua".into(), r#"
        local risk, files = require("risk"), require("files")
        sai.register_tool({name="reference",description="Reference",parameters={type="object"},execute=function(args)
            if args.path then return files.should_review(args.path) end
            return risk.evaluate(args.files)
        end})
    "#.into());
    let runtime = PluginRuntime::load(
        PluginPackage::new(package.manifest, sources).unwrap(),
        json!({}),
        Capabilities::default(),
        Arc::new(super::support::FixtureHost::default()),
    )
    .unwrap();
    for case in risks["risk_cases"].as_array().unwrap() {
        let text = runtime
            .call_tool(
                "reference",
                json!({"files":case["files"]}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            case["risk"],
            "{case}"
        );
    }
    for case in selections["selection_cases"].as_array().unwrap() {
        let text = runtime
            .call_tool(
                "reference",
                json!({"path":case["path"]}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            case["selected"],
            "{case}"
        );
    }
}
