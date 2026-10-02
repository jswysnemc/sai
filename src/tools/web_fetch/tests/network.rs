use super::{
    support::{response, server},
    *,
};

/// 【网页读取测试】【真实调用】参数为公开工具参数，返回经过原生注册入口执行的结果。
async fn call(args: Value) -> Result<String> {
    let mut registry = ToolRegistry::new();
    register(&mut registry);
    registry.call("web_fetch", &args.to_string()).await
}

/// 【网页读取测试】【格式转换】无参数；真实 HTTP 覆盖三种输出、HTML MIME 大小写与 XHTML。
#[tokio::test]
async fn web_fetch_converts_formats_and_sends_expected_headers() {
    let html = "<h1>标题</h1><p>some <strong>text</strong> <a href='/x'>link</a></p>";
    for content_type in [
        "text/html; charset=utf-8",
        "application/xhtml+xml",
        "TEXT/HTML",
    ] {
        for format in ["html", "text", "markdown"] {
            let expected = match format {
                "text" => html2text::from_read(html.as_bytes(), 120),
                "markdown" => html2md::parse_html(html),
                _ => html.into(),
            };
            let (url, task) = server(vec![response(
                200,
                &format!("Content-Type: {content_type}\r\n"),
                html.as_bytes(),
            )])
            .await;
            let actual = call(json!({"url":url,"format":format})).await.unwrap();
            assert_eq!(actual, expected);
            let requests = task.await.unwrap();
            let request = requests[0].to_lowercase();
            assert!(request.contains("accept-language: en-us,en;q=0.9"));
            assert!(request.contains("user-agent: mozilla/5.0"));
            assert!(request.starts_with("get / http/1.1"));
        }
    }
}

/// 【网页读取测试】【编码与截断】无参数；按声明解码中文，非 HTML 保持原文并按字符截断。
#[tokio::test]
async fn web_fetch_decodes_charset_and_clips_unicode() {
    let bytes = encoding_rs::GBK.encode("你好世界").0.into_owned();
    let (url, task) = server(vec![response(
        200,
        "Content-Type: text/plain; charset=gbk\r\n",
        &bytes,
    )])
    .await;
    let output = call(json!({"url":url,"max_chars":2})).await.unwrap();
    assert_eq!(
        output,
        "你好\n\n[content truncated from 4 chars to 2 chars]"
    );
    task.await.unwrap();
    let (url, task) = server(vec![response(
        200,
        "Content-Type: application/json\r\n",
        br#"{"answer":42}"#,
    )])
    .await;
    assert_eq!(call(json!({"url":url})).await.unwrap(), r#"{"answer":42}"#);
    task.await.unwrap();
}

/// 【网页读取测试】【拒绝与上限】无参数；零超时不连接、错误状态优先、分块正文也不能超过五 MiB。
#[tokio::test]
async fn web_fetch_enforces_timeouts_status_and_byte_limits_without_url_leaks() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/?token=fixture-secret",
        listener.local_addr().unwrap()
    );
    let error = call(json!({"url":url,"timeout":0}))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("timed out") && !error.contains("fixture-secret"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
    for status in [404, 503, 200] {
        let wire = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: 6000000\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let (url, task) = server(vec![wire]).await;
        let error = call(json!({"url":format!("{url}/?token=fixture-secret")}))
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains("fixture-secret"));
        if status == 200 {
            assert!(error.contains("exceeds 5MB limit"));
        } else {
            assert!(error.contains(&format!("({status})")));
        }
        task.await.unwrap();
    }
    let bytes = vec![b'x'; http::MAX_RESPONSE_BYTES + 1];
    let mut wire = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n",
        bytes.len()
    )
    .into_bytes();
    wire.extend_from_slice(&bytes);
    wire.extend_from_slice(b"\r\n0\r\n\r\n");
    let (url, task) = server(vec![wire]).await;
    assert!(call(json!({"url":url}))
        .await
        .unwrap_err()
        .to_string()
        .contains("exceeds 5MB limit"));
    task.await.unwrap();
}

/// 【网页读取测试】【重定向边界】无参数；允许十次跳转，第十一次拒绝且不回显目标查询参数。
#[tokio::test]
async fn web_fetch_follows_exactly_ten_redirects() {
    for success in [true, false] {
        let mut responses = vec![
            response(302, "Location: /next?token=fixture-secret\r\n", b"");
            if success { 10 } else { 11 }
        ];
        if success {
            responses.push(response(200, "Content-Type: text/plain\r\n", b"done"));
        }
        let (url, task) = server(responses).await;
        let result = call(json!({"url":url})).await;
        if success {
            assert_eq!(result.unwrap(), "done");
        } else {
            assert!(!result.unwrap_err().to_string().contains("fixture-secret"));
        }
        assert_eq!(task.await.unwrap().len(), 11);
    }
}

/// 【网页读取测试】【慢响应超时】无参数；正文迟迟不到时也受总超时约束，错误不带查询参数。
#[tokio::test]
async fn web_fetch_times_out_while_waiting_for_body() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/?token=fixture-secret",
        listener.local_addr().unwrap()
    );
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        socket.read(&mut request).await.unwrap();
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    });
    let error = call(json!({"url":url,"timeout":1})).await.unwrap_err();
    task.abort();
    let message = format!("{error:#}");
    assert!(message.contains("timed out"), "{message}");
    assert!(!message.contains("fixture-secret"));
}

/// 【网页读取测试】【可读正文】无参数；完整网页的脚本和 CSS 不污染 Markdown，原始 HTML 保持原样。
#[tokio::test]
async fn web_fetch_excludes_scripts_and_styles_from_readable_output() {
    let html = "<!doctype html><html><head><title>MetadataTitle</title><style>CSS_SECRET{color:red}</style></head><body><h1>Visible title</h1><script>JS_SECRET('ignored')</script><style>BODY_CSS{margin:0}</style><p>Visible body <a href='/docs'>Docs</a></p></body></html>";
    for format in ["markdown", "text", "html"] {
        let (url, task) = server(vec![response(
            200,
            "Content-Type: text/html\r\n",
            html.as_bytes(),
        )])
        .await;
        let output = call(json!({"url":url,"format":format})).await.unwrap();
        if format == "html" {
            assert_eq!(output, html);
        } else {
            assert!(
                output.contains("Visible title")
                    && output.contains("Visible body")
                    && output.contains("Docs")
            );
            for marker in ["MetadataTitle", "CSS_SECRET", "JS_SECRET", "BODY_CSS"] {
                assert!(!output.contains(marker), "{format}: {output}");
            }
        }
        task.await.unwrap();
    }
}
