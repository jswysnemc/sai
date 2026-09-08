use super::support::FixtureHost;
use super::web_search_support::{runtime, search, HTML, RESULT, TOOL};
use sai_plugin_runtime::InvocationContext;
use serde_json::json;
use std::sync::Arc;

/// 【搜索测试】【全链路回退】六个供应商按原有顺序尝试，错误与空结果按既有规则处理。
#[tokio::test]
async fn auto_search_keeps_the_complete_provider_fallback_order() {
    let host = Arc::new(FixtureHost::new(&[
        (500, "tinyfish unavailable"),
        (503, "tavily unavailable"),
        (502, "firecrawl unavailable"),
        (404, "anysearch unavailable"),
        (200, r#"{"results":[]}"#),
        (200, HTML),
    ]));
    let plugin = runtime(json!({}), host.clone());
    assert!(search(&plugin, "auto")
        .await
        .contains("DuckDuckGo HTML fallback"));
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| {
                reqwest::Url::parse(&request.url)
                    .unwrap()
                    .host_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>(),
        vec![
            "api.search.tinyfish.ai",
            "api.tavily.com",
            "api.firecrawl.dev",
            "api.anysearch.com",
            "search.example.test",
            "html.duckduckgo.com",
        ]
    );
}

/// 【搜索测试】【开关过滤】停用供应商不会发送请求，默认设置仍决定实际查询供应商。
#[tokio::test]
async fn disabled_providers_are_skipped_and_explicit_selection_is_rejected() {
    let host = Arc::new(FixtureHost::new(&[(200, RESULT)]));
    let plugin = runtime(
        json!({
            "tinyfish_enabled":false, "firecrawl_enabled":false, "anysearch_enabled":false,
            "default_provider":"tavily",
        }),
        host.clone(),
    );
    for args in [
        json!({"query":"Rust","provider":"tinyfish"}),
        json!({"query":"　 ","provider":"tavily"}),
        json!({"query":"Rust","provider":"unknown"}),
    ] {
        assert!(plugin
            .call_tool(TOOL, args, InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.requests.lock().unwrap().is_empty());
    let result = plugin
        .call_tool(TOOL, json!({"query":"Rust"}), InvocationContext::default())
        .await
        .unwrap();
    assert!(result.contains("**Provider**: Tavily"));
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【搜索测试】【空结果兼容】Tavily 的空结果保留原版标题响应，不额外触发后续供应商。
#[tokio::test]
async fn empty_tavily_results_keep_the_original_success_contract() {
    let host = Arc::new(FixtureHost::new(&[(200, r#"{"results":[]}"#)]));
    let plugin = runtime(json!({"tinyfish_enabled":false}), host.clone());
    assert_eq!(
        search(&plugin, "auto").await,
        "## Search results for: Rust\n**Provider**: Tavily\n"
    );
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【搜索测试】【整体失败】显式供应商失败后返回原有错误，不尝试其他已启用供应商。
#[tokio::test]
async fn explicit_provider_failure_does_not_change_to_another_provider() {
    let host = Arc::new(FixtureHost::new(&[(403, "denied")]));
    let plugin = runtime(json!({}), host.clone());
    let error = plugin
        .call_tool(
            TOOL,
            json!({"query":"Rust","provider":"tavily"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("no enabled web search provider succeeded"));
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【搜索测试】【无效地址回退】某个供应商地址不可解析时，仍能执行后续有效供应商。
#[tokio::test]
async fn malformed_provider_urls_do_not_disable_other_providers() {
    let host = Arc::new(FixtureHost::new(&[(200, RESULT)]));
    let plugin = runtime(
        json!({"tinyfish_base_url":"https://[invalid"}),
        host.clone(),
    );
    assert!(search(&plugin, "auto")
        .await
        .contains("**Provider**: Tavily"));
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}
