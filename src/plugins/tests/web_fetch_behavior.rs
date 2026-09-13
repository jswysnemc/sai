use super::support::runtime;
use super::web_fetch_http_support::{response, server};
use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

/// 【网页兼容测试】【结果契约】按既有字符计数规则生成预期结果
/// @param text 完整正文；limit 为字符上限
/// @returns 原正文或带完整字符数量的截断提示
fn expected(text: &str, limit: usize) -> String {
    let total = text.chars().count();
    if total <= limit {
        text.into()
    } else {
        format!(
            "{}\n\n[content truncated from {total} chars to {limit} chars]",
            text.chars().take(limit).collect::<String>()
        )
    }
}

/// 【网页兼容测试】【正文格式】真实 HTTP 响应保留既有 MIME、UTF-8 替换和字符裁剪行为
/// @returns 无，三种输出格式与原转换库一致
#[tokio::test]
async fn web_fetch_preserves_format_and_clipping_on_real_http() {
    let plugin = runtime("web-fetch", Arc::new(SaiPluginHost));
    let html = "<h1>标题</h1><p>some <strong>text</strong> <a href='/x'>link</a></p>";
    for content_type in [
        "text/html; charset=iso-8859-1",
        "application/xhtml+xml",
        "TEXT/HTML",
    ] {
        for format in ["html", "text", "markdown"] {
            let output = if content_type.contains("text/html") {
                match format {
                    "text" => html2text::from_read(html.as_bytes(), 120),
                    "markdown" => html2md::parse_html(html),
                    _ => html.to_string(),
                }
            } else {
                html.to_string()
            };
            let (url, task) = server(vec![response(
                200,
                &format!("Content-Type: {content_type}\r\n"),
                html.as_bytes(),
            )])
            .await;
            let actual = plugin
                .call_tool(
                    "web_fetch",
                    json!({"url":url,"format":format,"max_chars":8}),
                    InvocationContext::default(),
                )
                .await
                .unwrap();
            assert_eq!(actual, expected(&output, 8));
            let requests = task.await.unwrap();
            let request = requests[0].to_lowercase();
            assert!(request.contains("accept-language: en-us,en;q=0.9"));
            assert!(request.contains("user-agent: mozilla/5.0"));
        }
    }
    for (max_chars, limit) in [
        (json!(0), 1),
        (json!(-1), 24000),
        (json!(3.0), 24000),
        (json!(u64::MAX), 80000),
    ] {
        let body = b"a\xe4\xb8\xad\xffz";
        let (url, task) = server(vec![response(
            200,
            "Content-Type: text/plain; charset=iso-8859-1\r\n",
            body,
        )])
        .await;
        let actual = plugin
            .call_tool(
                "web_fetch",
                json!({"url":url,"max_chars":max_chars}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(actual, expected("a中\u{fffd}z", limit));
        task.await.unwrap();
    }
}

/// 【网页兼容测试】【请求拒绝】错误状态优先于正文上限，零超时不发起连接且错误不回显查询值
/// @returns 无，复用已注册实例仍可处理后续调用
#[tokio::test]
async fn web_fetch_handles_error_status_size_and_zero_timeout() {
    let plugin = runtime("web-fetch", Arc::new(SaiPluginHost));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/?token=fixture-secret",
        listener.local_addr().unwrap()
    );
    let error = plugin
        .call_tool(
            "web_fetch",
            json!({"url":url,"timeout":0}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
    for status in [404, 503, 200] {
        let wire = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: 6000000\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let (origin, task) = server(vec![wire]).await;
        let args: Value = json!({"url":format!("{origin}/?token=fixture-secret")});
        let error = plugin
            .call_tool("web_fetch", args, InvocationContext::default())
            .await
            .unwrap_err();
        let text = format!("{error:#}");
        assert!(!text.contains("fixture-secret"));
        if status == 200 {
            assert!(text.contains("exceeds 5MB limit"), "{text}");
        } else {
            assert!(text.contains(&format!("({status})")), "{text}");
        }
        task.await.unwrap();
    }
}
