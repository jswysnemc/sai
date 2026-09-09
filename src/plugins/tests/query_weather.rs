use super::query_support::{runtime, QueryHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::json;
use std::sync::Arc;

/// 【天气插件测试】【查询契约】地点编码、自动定位、Unicode 空白和原有结果前缀保持一致。
#[tokio::test]
async fn weather_preserves_location_encoding_and_response_text() {
    for (arguments, path) in [
        (json!({}), String::new()),
        (json!({"location":"　 \n"}), String::new()),
        (json!({"location":" New York "}), "/New%20York".into()),
        (
            json!({"location":" 北京/海淀 + "}),
            format!("/{}", urlencoding::encode("北京/海淀 +")),
        ),
    ] {
        let host = Arc::new(QueryHost::new(&[Ok((200, "　晴 +20°C 西风 北京 \n"))]));
        let plugin = runtime("weather", json!({}), host.clone());
        let text = plugin
            .call_tool("get_weather", arguments, InvocationContext::default())
            .await
            .unwrap();
        assert_eq!(
            text,
            "current weather(condition,temperature,wind,location): 晴 +20°C 西风 北京"
        );
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            format!("https://wttr.in{path}?format=%C+%t+%w+%l")
        );
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].max_bytes, 65536);
    }
}

/// 【天气插件测试】【失败边界】空正文、状态错误和超限正文均失败，错误不回显远端正文。
#[tokio::test]
async fn weather_rejects_empty_failed_and_oversized_responses() {
    let oversized = "x".repeat(65537);
    for (status, body, expected) in [
        (200, "　\n ", "weather response was empty"),
        (
            503,
            "response-not-for-error-output",
            "weather HTTP status 503",
        ),
        (200, oversized.as_str(), "byte limit"),
    ] {
        let host = Arc::new(QueryHost::new(&[Ok((status, body))]));
        let plugin = runtime("weather", json!({}), host);
        let error = plugin
            .call_tool("get_weather", json!({}), InvocationContext::default())
            .await
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("response-not-for-error-output"));
    }
}
