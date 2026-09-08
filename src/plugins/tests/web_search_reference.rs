use super::support::FixtureHost;
use super::web_search_support::{runtime, TOOL};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【搜索测试】【原版对照】使用原提交的 Rust 格式化与 HTML 解析结果核对真实 Lua 工具。
#[tokio::test]
async fn lua_search_matches_original_rust_formatting_and_html_parsing() {
    let fixtures = [
        include_str!("fixtures/web-search/tinyfish.json"),
        include_str!("fixtures/web-search/tavily.json"),
        include_str!("fixtures/web-search/firecrawl.json"),
        include_str!("fixtures/web-search/anysearch.json"),
        include_str!("fixtures/web-search/searxng.json"),
        include_str!("fixtures/web-search/duckduckgo.json"),
    ];
    let mut checked = 0;
    for fixture in fixtures {
        let data: Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            data["source_commit"],
            "44bddd0974ac6ce5c3b8b212debab57e70521bf1"
        );
        for case in data["cases"].as_array().unwrap() {
            let host = Arc::new(FixtureHost::new(&[(
                200,
                case["response"].as_str().unwrap(),
            )]));
            let plugin = runtime(json!({}), host.clone());
            let result = plugin
                .call_tool(TOOL, case["args"].clone(), InvocationContext::default())
                .await;
            if let Some(expected) = case["output"].as_str() {
                assert_eq!(
                    result.unwrap(),
                    expected,
                    "{} / {}",
                    case["provider"],
                    case["name"]
                );
            } else {
                assert!(
                    result.is_err(),
                    "{} / {} should reject the response",
                    case["provider"],
                    case["name"]
                );
            }
            assert_eq!(host.requests.lock().unwrap().len(), 1);
            checked += 1;
        }
    }
    assert_eq!(checked, 47);
}
