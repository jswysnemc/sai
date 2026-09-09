use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【游戏报告测试】【真实模块探针】仅增加测试入口，执行发布包原有报告整理和摘录函数。
/// @param host 无网络响应的测试宿主
/// @returns 具有纯文本对照入口的真实 Lua 运行时
fn reference_runtime(host: Arc<FixtureHost>) -> PluginRuntime {
    let original = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "linux-game-investigation")
        .unwrap();
    let mut sources = original.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
local report = require('report')
--- 【游戏报告测试】【对照入口】根据样本类型调用发布包的纯函数
--- @param input table 固定输入
--- @return string 实际输出
local function reference(input)
    if input.kind == 'strip' then return report.strip_preamble(input.content) end
    if input.kind == 'tool' then return report.tool_result(input.name, input.arguments, input.ok, input.output) end
    assert(input.kind == 'transcript', 'unknown report reference kind')
    return report.transcript(input.results, input.steps, input.max_steps)
end
sai.register_tool({name='reference',description='Compare original reports',parameters={type='object'},execute=reference})
"#);
    let grants = original.manifest.capabilities.clone();
    PluginRuntime::load(
        PluginPackage::new(original.manifest, sources).unwrap(),
        json!({}),
        grants,
        host,
    )
    .unwrap()
}

/// 【游戏报告测试】【原版对照】逐字验证章节提取、空白、UTF-8 字节边界和 6000 字符摘录，与第四轮 Rust 保持兼容。
#[tokio::test]
async fn lua_reports_and_unicode_excerpts_match_original_rust_results() {
    let source: Value = serde_json::from_str(include_str!(
        "fixtures/linux-game-investigation/report.json"
    ))
    .unwrap();
    assert_eq!(
        source["source_commit"],
        "bb5cb093ffcd62145d3c2a857193e2d310fc26c1"
    );
    let host = Arc::new(FixtureHost::default());
    let runtime = reference_runtime(host.clone());
    let cases = source["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 45);
    for case in cases {
        let output = runtime
            .call_tool(
                "reference",
                case["input"].clone(),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(output, case["output"].as_str().unwrap(), "{}", case["name"]);
    }
    assert!(host.requests.lock().unwrap().is_empty());
}
