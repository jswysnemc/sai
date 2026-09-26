use super::{
    support::{response, Server},
    *,
};
use std::time::Duration;

/// 【网页搜索测试】【真实请求回退】无参数；验证失败按固定顺序回退且完整发送请求正文。
#[tokio::test]
async fn web_search_falls_back_after_http_and_json_failures() {
    let server = Server::start(
        vec![
            response(503, "secret-body"),
            response(200, "not json"),
            response(
                200,
                r#"{"data":[{"title":"Rust","url":"https://example.test"}]}"#,
            ),
        ],
        Duration::ZERO,
    )
    .await;
    let config = WebSearchConfig {
        tinyfish_base_url: format!("{}/tiny", server.url),
        tinyfish_api_keys: vec!["tiny-key".into()],
        tavily_base_url: format!("{}/tavily", server.url),
        tavily_api_keys: vec!["tavily-key".into()],
        firecrawl_base_url: format!("{}/fire", server.url),
        firecrawl_api_keys: vec!["fire-key".into()],
        anysearch_enabled: false,
        searxng_enabled: false,
        duckduckgo_enabled: false,
        ..Default::default()
    };
    let mut registry = ToolRegistry::new();
    register(&mut registry, &config);
    let output = registry
        .call("web_search", r#"{"query":"Rust"}"#)
        .await
        .unwrap();
    assert!(output.contains("**Provider**: Firecrawl"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.0.as_str())
            .collect::<Vec<_>>(),
        ["/tiny?query=Rust", "/tavily", "/fire"]
    );
    assert_eq!(requests[1].2["authorization"], "Bearer tavily-key");
    let body: Value = serde_json::from_slice(&requests[2].3).unwrap();
    assert_eq!(body["query"], "Rust");
    assert_eq!(body["limit"], 5);
}

/// 【网页搜索测试】【显式失败】无参数；验证显式供应商不会越界回退且错误不泄露响应与凭据。
#[tokio::test]
async fn web_search_explicit_provider_failure_is_sanitized() {
    let server = Server::start(vec![response(401, "private-response")], Duration::ZERO).await;
    let config = WebSearchConfig {
        tavily_base_url: format!("{}/?token=private-query", server.url),
        tavily_api_keys: vec!["private-key".into()],
        ..Default::default()
    };
    let error = search(json!({"query":"Rust","provider":"tavily"}), config)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "no enabled web search provider succeeded"
    );
    assert_eq!(server.requests.lock().unwrap().len(), 1);
}

/// 【网页搜索测试】【超时回退】无参数；验证请求期限包含等待响应，并允许后续供应商成功。
#[tokio::test]
async fn web_search_times_out_then_uses_next_provider() {
    let slow = Server::start(vec![response(200, "{}")], Duration::from_secs(3)).await;
    let fast = Server::start(
        vec![response(200, r#"{"results":[{"title":"Ready"}]}"#)],
        Duration::ZERO,
    )
    .await;
    let config = WebSearchConfig {
        tinyfish_base_url: slow.url.clone(),
        tinyfish_api_keys: vec!["tiny-key".into()],
        tavily_base_url: fast.url.clone(),
        tavily_api_keys: vec!["tavily-key".into()],
        timeout_seconds: 1,
        ..Default::default()
    };
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        search(json!({"query":"Rust"}), config),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.contains("Ready"));
}

/// 【网页搜索测试】【响应边界】无参数；验证分块大响应、字符集解码和跨来源重定向。
#[tokio::test]
async fn web_search_http_limits_decode_and_block_cross_origin_credentials() {
    let destination = Server::start(vec![response(200, "private")], Duration::ZERO).await;
    let redirect = axum::response::Response::builder()
        .status(307)
        .header("location", &destination.url)
        .body(axum::body::Body::empty())
        .unwrap();
    let (encoded, _, _) = encoding_rs::GBK.encode("中文搜索");
    let gbk = axum::response::Response::builder()
        .header("content-type", "text/html; charset=gbk")
        .body(axum::body::Body::from(encoded.into_owned()))
        .unwrap();
    let stream = futures_util::stream::iter([Ok::<_, std::io::Error>(vec![
        b'x';
        http::MAX_RESPONSE_BYTES
            + 1
    ])]);
    let large = axum::response::Response::new(axum::body::Body::from_stream(stream));
    let server = Server::start(vec![redirect, gbk, large], Duration::ZERO).await;
    let client = http::client(2).unwrap();
    assert!(
        http::execute(client.get(&server.url).header("X-API-Key", "secret"))
            .await
            .is_err()
    );
    assert!(destination.requests.lock().unwrap().is_empty());
    assert_eq!(
        http::execute(client.get(&server.url)).await.unwrap(),
        "中文搜索"
    );
    assert!(http::execute(client.get(&server.url))
        .await
        .unwrap_err()
        .to_string()
        .contains("byte limit"));
}
