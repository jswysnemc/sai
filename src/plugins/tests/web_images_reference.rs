use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

const REFERENCE: &str = r#"
local ddg, bing = require('providers.duckduckgo'), require('providers.bing')
local ranking, strings = require('ranking'), require('strings')
local metadata, screening, result = require('image_metadata'), require('screening'), require('result')
--- 【搜图测试】【对照入口】调用真实发布模块并返回原版样本对应的结果
--- @param args table 操作名称及输入样本
--- @return any 原规则输出
local function evaluate(args)
    local op = args.op
    if op == 'vqd' then return ddg.vqd(args.text) or sai.json.null
    elseif op == 'ddg' then local ok, value = pcall(ddg.parse, args.text, args.count); if ok then return value else return {error=true} end
    elseif op == 'bing' then return bing.parse(args.text, args.count)
    elseif op == 'terms' then return ranking.terms(args.query)
    elseif op == 'score' then return ranking.score(args.query, args.item)
    elseif op == 'rank' then return ranking.rank(args.query, ranking.dedupe(args.items))
    elseif op == 'limits' then return sai.json.array({ranking.pool_limit(args.count),ranking.probe_limit(args.count)})
    elseif op == 'strings' then return {clean=strings.clean(args.text,args.count),url=strings.url(args.text),unescape=strings.unescape(args.text),host=strings.host(args.text) or sai.json.null}
    elseif op == 'mime' or op == 'dimensions' then
        local body = sai.binary.decode_base64(args.bytes)
        local value
        if op == 'mime' then value = metadata.mime(body,args.type,args.url) or sai.json.null
        else local width,height = metadata.dimensions(body,args.type); value = sai.json.array({width,height}) end
        body:close()
        return value
    elseif op == 'screen' then return screening.parse(args.text,{provider_id='vision-fixture',model='vision-model'})
    elseif op == 'prompt' then return screening.prompt(args.query,args.item)
    elseif op == 'bytes' then return result.bytes(args.bytes)
    elseif op == 'stored' then return result.image(args.item)
    end
    error('unknown reference operation')
end
sai.register_tool({name='reference',description='Compare original behavior',parameters={type='object'},execute=function(args)
    return {value=evaluate(args)}
end})
"#;

/// 【搜图测试】【原版对照】真实 Lua 模块必须匹配从原 Rust 纯函数生成的固定结果。
/// @returns 无；失败时报告样本序号和完整输入
#[tokio::test]
async fn web_image_rules_match_original_rust_reference() {
    let package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "web-images")
        .unwrap();
    let mut sources = package.sources().clone();
    sources.insert("init.lua".into(), REFERENCE.into());
    let package = PluginPackage::new(package.manifest, sources).unwrap();
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Default::default(),
        Arc::new(FixtureHost::default()),
    )
    .unwrap();
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/web_images_reference.json")).unwrap();
    assert!(cases.len() > 250);
    let mut differences = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let output = plugin
            .call_tool(
                "reference",
                case["input"].clone(),
                InvocationContext::default(),
            )
            .await
            .unwrap_or_else(|error| panic!("case {index} {}: {error:#}", case["input"]));
        // 1. 【搜图测试】【结果类型】统一容器保留顶层空值和字符串，避免工具文本转换影响对照
        let actual = serde_json::from_str::<Value>(&output)
            .unwrap_or_else(|error| panic!("case {index} {}: {error}: {output}", case["input"]))
            ["value"]
            .clone();
        let matches = if case["input"]["op"] == "score" {
            actual.as_f64() == case["expected"].as_f64()
        } else {
            actual == case["expected"]
        };
        if !matches {
            differences.push(format!(
                "case {index}: {}\nactual: {actual}\nexpected: {}",
                case["input"], case["expected"]
            ));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
