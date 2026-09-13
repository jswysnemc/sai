use super::image_support::ImageHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【表情对照】【真实模块】在发布源码快照中追加测试入口，不替换任何业务函数
/// @returns 可查询原版样本所涉及函数的独立运行时
fn reference_runtime() -> PluginRuntime {
    let package = super::example_support::package("memes");
    let mut manifest = package.manifest.clone();
    let mut sources = package.sources().clone();
    manifest.entry = "reference.lua".into();
    sources.insert(
        "reference.lua".into(),
        include_str!("fixtures/memes_reference.lua").into(),
    );
    let grants = manifest.capabilities.clone();
    PluginRuntime::load(
        PluginPackage::new(manifest, sources).unwrap(),
        json!({}),
        grants,
        Arc::new(ImageHost::default()),
    )
    .unwrap()
}

/// 【表情对照】【冻结样本】逐条比较实际 Lua 规则和冻结原生函数，错误保留业务原因
/// @param document 含来源提交、源码摘要和原版结果的样本集
/// @returns 无；任何行为差异都使测试失败
async fn compare(document: &str) {
    let fixtures: Value = serde_json::from_str(document).unwrap();
    assert_eq!(
        fixtures["source_commit"],
        "90a911c8859d1bef72e7d983e865a815a0abab47"
    );
    let runtime = reference_runtime();
    for case in fixtures["cases"].as_array().unwrap() {
        let mut input = case.clone();
        input.as_object_mut().unwrap().remove("expected");
        let output = runtime
            .call_tool("reference", input, InvocationContext::default())
            .await;
        let expected = &case["expected"];
        if expected["ok"] == true {
            let output = output.unwrap_or_else(|error| panic!("{}: {error:#}", case["label"]));
            let actual: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(actual, expected["value"], "{}", case["label"]);
        } else {
            let error = output.expect_err(case["label"].as_str().unwrap());
            assert!(
                format!("{error:#}").contains(expected["error"].as_str().unwrap()),
                "{}: {error:#}",
                case["label"]
            );
        }
    }
}

/// 【表情对照】【文本规则】覆盖 Unicode、ASCII 归一化、前缀方向和稳定计分
/// @returns 无；211 条原版样本保持兼容
#[tokio::test]
async fn memes_match_native_text_rules() {
    compare(include_str!("fixtures/memes_rules.json")).await;
}

/// 【表情对照】【元数据规则】覆盖手工、视觉及局部更新的空值和类型边界
/// @returns 无；34 条原版样本保持兼容
#[tokio::test]
async fn memes_match_native_metadata_rules() {
    compare(include_str!("fixtures/memes_metadata.json")).await;
}

/// 【表情对照】【原始整数】浮点表示、完整无符号整数、尺寸优先级和库名保持兼容
/// @returns 无；25 条原版样本保持兼容
#[tokio::test]
async fn memes_match_native_argument_rules() {
    compare(include_str!("fixtures/memes_arguments.json")).await;
}
