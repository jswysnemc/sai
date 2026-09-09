use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【输入法报告测试】【真实模块】为实际发布包添加测试入口，直接调用原有报告和提示词函数。
/// @param host 无网络响应的测试宿主
/// @returns 保留正式源码及纯函数测试入口的 Lua 运行时
fn reference_runtime(host: Arc<FixtureHost>) -> PluginRuntime {
    let original = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "input-method-investigation")
        .unwrap();
    let mut sources = original.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
local report = require('report')
--- 【输入法报告测试】【对照入口】根据固定样本类型调用发布包中的纯函数
--- @param input table 固定输入
--- @return string 实际输出
local function reference(input)
    if input.kind == 'strip' then return report.strip_preamble(input.content) end
    if input.kind == 'clip' then return report.clip_inline(input.content, input.max_chars) end
    if input.kind == 'tool' then return report.tool_result(input.name, input.arguments, input.ok, input.output) end
    if input.kind == 'prompt' then
        return report.input_prompt(input.issue, type(input.target) == 'string' and input.target or nil)
    end
    assert(input.kind == 'transcript', 'unknown report reference kind')
    return report.transcript(input.results, input.steps, input.max_steps)
end
sai.register_tool({name='reference',description='Compare original input method reports',parameters={type='object'},execute=reference})
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

/// 【输入法报告测试】【原版对照】验证五个原 Rust 纯函数生成的固定样本，覆盖字节边界、Unicode 截断和目标缺省值。
#[tokio::test]
async fn lua_reports_excerpts_and_prompts_match_original_rust_results() {
    let fixtures = [
        include_str!("fixtures/input-method-investigation/reports.json"),
        include_str!("fixtures/input-method-investigation/excerpts.json"),
        include_str!("fixtures/input-method-investigation/prompts.json"),
    ];
    let host = Arc::new(FixtureHost::default());
    let runtime = reference_runtime(host.clone());
    let mut total = 0;
    for text in fixtures {
        let source: Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            source["source_commit"],
            "757cb7145d60bdd75efda44795141355f3794c82"
        );
        for case in source["cases"].as_array().unwrap() {
            let output = runtime
                .call_tool(
                    "reference",
                    case["input"].clone(),
                    InvocationContext::default(),
                )
                .await
                .unwrap();
            assert_eq!(output, case["output"].as_str().unwrap(), "{}", case["name"]);
            total += 1;
        }
    }
    assert_eq!(total, 75);
    assert!(host.requests.lock().unwrap().is_empty());
}
