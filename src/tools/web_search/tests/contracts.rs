use super::*;
use reqwest::Client;

/// 【网页搜索测试】【样本配置】无参数；返回不读取真实供应商环境变量的显式测试配置。
fn config() -> WebSearchConfig {
    WebSearchConfig {
        tinyfish_api_keys: vec!["tiny-key".into()],
        tavily_api_keys: vec!["tavily-key".into()],
        firecrawl_api_keys: vec!["fire-key".into()],
        anysearch_api_keys: vec!["any-key".into()],
        searxng_base_url: "https://search.example.test".into(),
        ..Default::default()
    }
}

/// 【网页搜索测试】【原版样本】无参数；逐条比较六个供应商的四十七个历史输出和拒绝结果。
#[test]
fn web_search_matches_all_frozen_native_and_lua_outputs() {
    let mut count = 0;
    for fixture in [
        include_str!("../../../plugins/tests/fixtures/web-search/tinyfish.json"),
        include_str!("../../../plugins/tests/fixtures/web-search/tavily.json"),
        include_str!("../../../plugins/tests/fixtures/web-search/firecrawl.json"),
        include_str!("../../../plugins/tests/fixtures/web-search/anysearch.json"),
        include_str!("../../../plugins/tests/fixtures/web-search/searxng.json"),
        include_str!("../../../plugins/tests/fixtures/web-search/duckduckgo.json"),
    ] {
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = SearchInput::parse(&case["args"], &config()).unwrap();
            let result = providers::render_response(
                case["provider"].as_str().unwrap(),
                &input,
                case["response"].as_str().unwrap(),
            );
            if let Some(expected) = case["output"].as_str() {
                assert_eq!(
                    result.unwrap(),
                    expected,
                    "{} / {}",
                    case["provider"],
                    case["name"]
                );
            } else {
                assert!(result.is_err(), "{} / {}", case["provider"], case["name"]);
            }
            count += 1;
        }
    }
    assert_eq!(count, 47);
}

/// 【网页搜索测试】【供应商协议】无参数；验证查询编码、认证头、地区回退与各 POST 选项。
#[test]
fn web_search_requests_preserve_provider_contracts() {
    let config = WebSearchConfig {
        tinyfish_base_url: "https://tiny.example.test/search?tenant=one#section".into(),
        tinyfish_default_location: " GB ".into(),
        tinyfish_default_language: "en".into(),
        tavily_search_depth: "advanced".into(),
        tavily_include_answer: true,
        firecrawl_only_main_content: false,
        ..config()
    };
    let input = SearchInput::parse(&json!({"query":"　输入法 Rust *~&+ ","max_results":99,"location":"　", "language":" zh-CN "}), &config).unwrap();
    assert_eq!(input.max_results, 10);
    assert!(SearchInput::parse(&json!({"query":"　 "}), &config).is_err());
    let client = Client::new();
    let tiny = providers::build_request(&client, "tinyfish", &input, &config)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(tiny.method(), "GET");
    assert_eq!(tiny.headers()["x-api-key"], "tiny-key");
    assert_eq!(tiny.url().fragment(), Some("section"));
    assert!(tiny.url().query().unwrap().contains("+*%7E%26%2B"));
    assert_eq!(
        tiny.url()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect::<Vec<_>>(),
        vec![
            ("tenant".into(), "one".into()),
            ("query".into(), input.query.clone()),
            ("location".into(), "GB".into()),
            ("language".into(), "zh-CN".into())
        ]
    );
    for (provider, key, expected) in [
        (
            "tavily",
            "tavily-key",
            json!({"query":input.query,"max_results":10,"search_depth":"advanced","include_answer":true,"include_raw_content":"markdown"}),
        ),
        (
            "firecrawl",
            "fire-key",
            json!({"query":input.query,"limit":10,"sources":[{"type":"web"}],"scrapeOptions":{"formats":[{"type":"markdown"}],"onlyMainContent":false}}),
        ),
        (
            "anysearch",
            "any-key",
            json!({"query":input.query,"max_results":10}),
        ),
    ] {
        let request = providers::build_request(&client, provider, &input, &config)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.method(), "POST");
        assert_eq!(request.headers()["authorization"], format!("Bearer {key}"));
        assert_eq!(
            serde_json::from_slice::<Value>(request.body().unwrap().as_bytes().unwrap()).unwrap(),
            expected
        );
    }
    let ddg = providers::build_request(&client, "duckduckgo", &input, &config)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(ddg.url().host_str(), Some("html.duckduckgo.com"));
    assert!(ddg.headers()["user-agent"]
        .to_str()
        .unwrap()
        .starts_with("Mozilla/5.0"));
    assert_eq!(
        provider_order(&config, "auto"),
        [
            "tinyfish",
            "tavily",
            "firecrawl",
            "anysearch",
            "searxng",
            "duckduckgo"
        ]
    );
    assert_eq!(provider_order(&config, "script"), ["duckduckgo"]);
    let mut disabled = config;
    disabled.tavily_enabled = false;
    assert!(provider_order(&disabled, "tavily").is_empty());
}

/// 【网页搜索测试】【环境凭据】无参数；验证显式密钥、环境引用和缺省环境变量优先级。
#[test]
fn web_search_credentials_preserve_environment_references() {
    let variable = format!("SAI_SEARCH_FIXTURE_{}", uuid::Uuid::new_v4().simple());
    std::env::set_var(&variable, " env-key ");
    assert_eq!(
        providers::first_api_key(&[" direct-key ".into()], &variable).unwrap(),
        "direct-key"
    );
    assert_eq!(
        providers::first_api_key(&[format!("$env: {variable}")], "SAI_SEARCH_UNUSED").unwrap(),
        "env-key"
    );
    assert_eq!(
        providers::first_api_key(&[" ".into(), "$env:SAI_SEARCH_MISSING".into()], &variable)
            .unwrap(),
        "env-key"
    );
    std::env::remove_var(&variable);
    assert!(providers::first_api_key(&[], &variable).is_err());
}
