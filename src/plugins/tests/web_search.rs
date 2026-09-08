use super::support::FixtureHost;
use super::web_search_support::{runtime, search, HTML, RESULT, TOOL};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【搜索测试】【TinyFish 契约】保留查询编码、地区回退、结果上限以及原有 120 秒配置。
#[tokio::test]
async fn tinyfish_preserves_query_parameters_and_request_deadline() {
    let host = Arc::new(FixtureHost::new(&[(
        200,
        r#"{"results":[{"title":"一"},{"title":"二"},{"title":"三"}]}"#,
    )]));
    let plugin = runtime(
        json!({
            "tinyfish_base_url":"https://tiny.example.test/search?tenant=one#section",
            "tinyfish_default_location":" GB ", "tinyfish_default_language":"en",
            "timeout_seconds":120,
        }),
        host.clone(),
    );
    let query = "输入法 Rust *~&+";
    let output = plugin
        .call_tool(
            TOOL,
            json!({
                "query":format!("　{query} "), "provider":"tinyfish", "max_results":2,
                "location":"　", "language":" zh-CN ",
            }),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        output,
        format!(
            "## Search results for: {query}\n**Provider**: TinyFish\n\n### 1. 一\n\n### 2. 二\n"
        )
    );
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.headers["x-api-key"], "tiny-key");
    assert_eq!(request.timeout_ms, 120_000);
    assert!(request.body.is_none());
    let url = reqwest::Url::parse(&request.url).unwrap();
    assert_eq!(
        url.query_pairs().collect::<Vec<_>>(),
        vec![
            ("tenant".into(), "one".into()),
            ("query".into(), query.into()),
            ("location".into(), "GB".into()),
            ("language".into(), "zh-CN".into()),
        ]
    );
    assert!(url.query().unwrap().contains("+*%7E%26%2B"));
    assert_eq!(url.fragment(), Some("section"));
}

/// 【搜索测试】【Tavily 契约】只读 POST 保留认证、搜索选项和数量归一化。
#[tokio::test]
async fn tavily_uses_authorized_post_with_the_original_options() {
    let host = Arc::new(FixtureHost::new(&[(200, RESULT)]));
    let plugin = runtime(
        json!({
            "tavily_base_url":"https://proxy.example.test/tavily/search?tenant=one",
            "tavily_search_depth":"advanced", "tavily_include_answer":true,
            "tavily_include_raw_content":false,
        }),
        host.clone(),
    );
    let output = plugin
        .call_tool(
            TOOL,
            json!({
                "query":"Rust", "provider":"tavily", "max_results":99,
            }),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(output, "## Search results for: Rust\n**Provider**: Tavily\n\n### 1. Rust\n**URL**: https://www.rust-lang.org\n**Snippet**: 语言文档\n");
    let requests = host.requests.lock().unwrap();
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.headers["authorization"], "Bearer tavily-key");
    assert_eq!(request.headers["content-type"], "application/json");
    assert_eq!(
        serde_json::from_str::<Value>(request.body.as_ref().unwrap()).unwrap(),
        json!({
            "query":"Rust", "max_results":10, "search_depth":"advanced",
            "include_answer":true, "include_raw_content":false,
        })
    );
}

/// 【搜索测试】【Firecrawl 契约】保留 data 数组、元数据字段与 Markdown 抓取选项。
#[tokio::test]
async fn firecrawl_preserves_metadata_and_all_returned_results() {
    let response = json!({"data":[
        {"metadata":{"title":"Metadata title","sourceURL":"https://example.test/source"},"markdown":"正文"},
        {"title":"Second","metadata":{"url":"https://example.test/second"},"description":"摘要"},
    ]}).to_string();
    let host = Arc::new(FixtureHost::new(&[(200, &response)]));
    let plugin = runtime(json!({"firecrawl_only_main_content":false}), host.clone());
    let output = plugin
        .call_tool(
            TOOL,
            json!({
                "query":"Rust", "provider":"firecrawl", "max_results":1,
            }),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(output, "## Search results for: Rust\n**Provider**: Firecrawl\n\n### 1. Metadata title\n**URL**: https://example.test/source\n**Content**: 正文\n\n### 2. Second\n**URL**: https://example.test/second\n**Snippet**: 摘要\n");
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests[0].headers["authorization"], "Bearer firecrawl-key");
    assert_eq!(
        serde_json::from_str::<Value>(requests[0].body.as_ref().unwrap()).unwrap(),
        json!({
            "query":"Rust", "limit":1, "sources":[{"type":"web"}],
            "scrapeOptions":{"formats":[{"type":"markdown"}],"onlyMainContent":false},
        })
    );
}

/// 【搜索测试】【AnySearch 格式】显式 null 不触发备用字段，Unicode 裁剪边界与原版一致。
#[tokio::test]
async fn anysearch_keeps_field_precedence_and_unicode_clipping() {
    let snippet = "界".repeat(500);
    let raw = "文".repeat(800);
    let response = json!({"results":[
        {"title":null,"metadata":{"title":"must not replace null"},"url":false,"content":false,"description":"must not replace false"},
        {"title":"Exact","snippet":snippet,"raw_content":raw},
        {"title":"Long","snippet":format!("{snippet}界"),"markdown":format!("{raw}文")},
    ]}).to_string();
    let host = Arc::new(FixtureHost::new(&[(200, &response)]));
    let plugin = runtime(json!({}), host.clone());
    let output = search(&plugin, "anysearch").await;
    assert_eq!(output, format!("## Search results for: Rust\n**Provider**: AnySearch\n\n### 1. Untitled\n\n### 2. Exact\n**Snippet**: {snippet}\n**Content**: {raw}\n\n### 3. Long\n**Snippet**: {snippet}...\n**Content**: {raw}...\n"));
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].headers["authorization"], "Bearer anysearch-key");
    assert_eq!(
        serde_json::from_str::<Value>(requests[0].body.as_ref().unwrap()).unwrap(),
        json!({"query":"Rust","max_results":5})
    );
}

/// 【搜索测试】【SearXNG 契约】自定义实例保留语言与安全搜索参数，且只返回指定数量。
#[tokio::test]
async fn searxng_uses_the_configured_instance_and_limits_results() {
    let host = Arc::new(FixtureHost::new(&[(
        200,
        r#"{"results":[{"title":"First"},{"title":"Second"}]}"#,
    )]));
    let plugin = runtime(
        json!({
            "searxng_base_url":"https://search.example.test/base///",
            "searxng_language":" zh-CN ", "searxng_safe_search":2, "max_results":1,
        }),
        host.clone(),
    );
    let output = search(&plugin, "searxng").await;
    assert!(output.contains("### 1. First"));
    assert!(!output.contains("Second"));
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests[0].url,
        "https://search.example.test/base/search?q=Rust&format=json&language=zh-CN&safesearch=2"
    );
    assert_eq!(requests[0].headers["accept"], "application/json");
}

/// 【搜索测试】【DuckDuckGo 契约】保留 HTML 实体、Unicode 空白和旧版 script 别名。
#[tokio::test]
async fn duckduckgo_preserves_html_output_and_legacy_alias() {
    let host = Arc::new(FixtureHost::new(&[(200, HTML)]));
    let plugin = runtime(json!({}), host.clone());
    let output = search(&plugin, "script").await;
    assert_eq!(output, "## Search results for: Rust\n**Provider**: DuckDuckGo HTML fallback\n\n### 1. 输入法 Rust\n**URL**: https://example.test/?a=1&b=2\n**Snippet**: 中文 与 Lua\n");
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests[0].url, "https://html.duckduckgo.com/html/?q=Rust");
    assert!(requests[0].headers["user-agent"].starts_with("Mozilla/5.0"));
}
