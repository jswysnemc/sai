use super::query_support::{runtime, QueryHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【百科插件测试】【站点与搜索】支持原站点别名，搜索模式返回同一 JSON 数据并保持查询参数。
#[tokio::test]
async fn moegirl_search_preserves_sites_and_json_payloads() {
    for (site, origin) in [
        ("zh", "https://zh.moegirl.org.cn"),
        ("cn", "https://zh.moegirl.org.cn"),
        ("", "https://zh.moegirl.org.cn"),
        ("uk", "https://moegirl.uk"),
        ("ja", "https://ja.moegirl.org"),
    ] {
        let response = json!(["原神 +", ["结果"], ["摘要"], ["https://example.test/page"]]);
        let body = response.to_string();
        let host = Arc::new(QueryHost::new(&[Ok((200, &body))]));
        let plugin = runtime("moegirl", json!({}), host.clone());
        let output = plugin
            .call_tool(
                "query_moegirl",
                json!({"mode":"search","site":site,"query":"　原神 + "}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&output).unwrap(), response);
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            format!(
                "{origin}/api.php?action=opensearch&search={}&limit=5&namespace=0&format=json",
                urlencoding::encode("原神 +")
            )
        );
        assert_eq!(requests[0].headers["user-agent"], "sai/0.1");
        assert_eq!(requests[0].timeout_ms, 10000);
    }
}

/// 【百科插件测试】【自动模式】优先显式标题，其次首个搜索标题，无命中时使用原查询读取。
#[tokio::test]
async fn moegirl_auto_mode_preserves_title_precedence_and_no_hit_fallback() {
    for (arguments, search, expected_title) in [
        (
            json!({"title":" 明确标题 ","query":"ignored"}),
            None,
            "明确标题",
        ),
        (
            json!({"query":"query"}),
            Some(r#"["query",[" First title "],[],[]]"#),
            " First title ",
        ),
        (
            json!({"query":"query"}),
            Some(r#"["query",[],[],[]]"#),
            "query",
        ),
        (json!({"query":"query"}), Some("null"), "query"),
        (json!({"mode":"page","query":"query"}), None, "query"),
    ] {
        let mut responses = Vec::new();
        if let Some(search) = search {
            responses.push(Ok((200, search)));
        }
        responses.push(Ok((200, "<h2>正文</h2><p><a href='/条目'>链接</a></p>")));
        let host = Arc::new(QueryHost::new(&responses));
        let plugin = runtime("moegirl", json!({}), host.clone());
        let output = plugin
            .call_tool("query_moegirl", arguments, InvocationContext::default())
            .await
            .unwrap();
        assert!(output.starts_with(&format!(
            "Source: https://zh.moegirl.org.cn/{}\n\n",
            urlencoding::encode(expected_title)
        )));
        assert!(output.contains("[链接](/条目)"));
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), if search.is_some() { 2 } else { 1 });
        assert_eq!(
            requests.last().unwrap().url,
            format!(
                "https://zh.moegirl.org.cn/rest.php/v1/page/{}/html",
                urlencoding::encode(expected_title)
            )
        );
        assert_eq!(requests.last().unwrap().max_bytes, 524288);
    }
}

/// 【百科插件测试】【页面回退】传输与正文大小失败进入解析接口，明确 HTTP 错误不回退。
#[tokio::test]
async fn moegirl_page_fallback_distinguishes_transport_size_and_status() {
    let oversized = "x".repeat(524289);
    for first in [
        Err("transport fixture failed"),
        Ok((200, oversized.as_str())),
    ] {
        let host = Arc::new(QueryHost::new(&[
            first,
            Ok((200, r#"{"parse":{"text":{"*":"<p>API 正文</p>"}}}"#)),
        ]));
        let plugin = runtime("moegirl", json!({}), host.clone());
        let output = plugin
            .call_tool(
                "query_moegirl",
                json!({"mode":"page","title":"页面"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert!(output.ends_with("API 正文"));
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].url.contains("action=parse&page="));
    }
    let host = Arc::new(QueryHost::new(&[Ok((404, "not found")), Ok((200, "{}"))]));
    let plugin = runtime("moegirl", json!({}), host.clone());
    let error = plugin
        .call_tool(
            "query_moegirl",
            json!({"title":"页面"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Moegirlpedia HTTP status 404"));
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【百科插件测试】【正文边界】按原版两万个 Unicode 字符截断，恰好达到边界时不追加标记。
#[tokio::test]
async fn moegirl_markdown_clipping_keeps_unicode_and_exact_boundaries() {
    for count in [0, 19999, 20000, 20001] {
        let body = format!("<p>{}</p>", "界".repeat(count));
        let plugin = runtime(
            "moegirl",
            json!({}),
            Arc::new(QueryHost::new(&[Ok((200, &body))])),
        );
        let output = plugin
            .call_tool(
                "query_moegirl",
                json!({"title":"页面"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        let expected = if count > 20000 {
            format!("{}\n...[truncated to 20000 chars]", "界".repeat(20000))
        } else {
            "界".repeat(count)
        };
        assert_eq!(
            output,
            format!(
                "Source: https://zh.moegirl.org.cn/{}\n\n{expected}",
                urlencoding::encode("页面")
            )
        );
    }
}

/// 【百科插件测试】【缺失内容】空参数在联网前失败，回退 API 中无有效正文时保留明确错误。
#[tokio::test]
async fn moegirl_missing_titles_and_api_content_fail_explicitly() {
    let host = Arc::new(QueryHost::new(&[]));
    let plugin = runtime("moegirl", json!({}), host.clone());
    for args in [
        json!({}),
        json!({"title":"　","mode":"page"}),
        json!({"mode":"search"}),
    ] {
        let error = plugin
            .call_tool("query_moegirl", args, InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("query or title is required"));
    }
    assert!(host.requests.lock().unwrap().is_empty());
    for body in [
        "null",
        "false",
        "{}",
        r#"{"parse":{"text":{"*":"　 "}}}"#,
        r#"{"parse":{"text":{"*":false}}}"#,
    ] {
        let host = Arc::new(QueryHost::new(&[
            Err("transport fixture failed"),
            Ok((200, body)),
        ]));
        let plugin = runtime("moegirl", json!({}), host);
        let error = plugin
            .call_tool(
                "query_moegirl",
                json!({"title":"页面"}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("Moegirlpedia page not found or returned empty content")
        );
    }
}
