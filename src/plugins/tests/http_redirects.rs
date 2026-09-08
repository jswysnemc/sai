use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{HttpRequest, PluginHost};
use sai_plugin_runtime::Capabilities;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【网络测试】【请求读取】读取完整请求头及声明长度的正文，避免 TCP 分片影响断言。
/// @param stream 已接受的本地连接
/// @returns UTF-8 请求报文
async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let count = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        assert!(count > 0, "request ended before the complete body");
        bytes.extend_from_slice(&chunk[..count]);
        assert!(bytes.len() <= 64 * 1024);
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8(bytes).unwrap()
}

/// 【网络测试】【响应服务】依次返回固定报文并记录收到的完整请求。
/// @param responses 按连接顺序发送的 HTTP 响应
/// @returns 精确来源和可等待的请求记录任务
async fn server(responses: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut received = Vec::new();
        for response in responses {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            received.push(read_request(&mut stream).await);
            stream.write_all(response.as_bytes()).await.unwrap();
        }
        received
    });
    (origin, task)
}

/// 【网络测试】【重定向响应】生成不含正文且主动关闭连接的重定向报文。
/// @param status HTTP 状态；target 为目标地址或相对路径
/// @returns 完整响应文本
fn redirect(status: u16, target: &str) -> String {
    format!("HTTP/1.1 {status} Redirect\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// 【网络测试】【读取响应】生成固定成功报文，无参数。
/// @returns 内容为 ok 的完整响应
fn ok_response() -> String {
    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".into()
}

/// 【网络测试】【查询请求】构造带认证头的只读 POST 查询。
/// @param origin 目标来源
/// @returns 发送到 /search 的请求
fn request(origin: &str) -> HttpRequest {
    HttpRequest {
        url: format!("{origin}/search"),
        method: "POST".into(),
        headers: BTreeMap::from([
            ("authorization".into(), "Bearer header-secret".into()),
            ("x-api-key".into(), "custom-secret".into()),
            ("content-type".into(), "application/json".into()),
        ]),
        body: Some("{\"query\":\"Rust\"}".into()),
        max_bytes: 1024,
        timeout_ms: 2000,
    }
}

/// 【网络测试】【查询声明】授权一个来源及其指定 POST 查询路径。
/// @param origin 来源；paths 为相对端点路径
/// @returns 可用于只读调用的网络授权
fn grants(origin: &str, paths: &[&str]) -> Capabilities {
    Capabilities {
        http: [origin.to_string()].into(),
        http_read_only_post: paths.iter().map(|path| format!("{origin}{path}")).collect(),
    }
}

/// 【网络测试】【POST 重定向】只读 POST 不能通过同一来源的 307 跳转写入其他端点。
#[tokio::test]
async fn post_redirects_cannot_escape_the_authorized_query_path() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(redirect(307, "/delete?token=private-query").as_bytes())
            .await
            .unwrap();
        drop(stream);
        assert!(
            tokio::time::timeout(Duration::from_millis(80), listener.accept())
                .await
                .is_err()
        );
    });
    let error = SaiPluginHost
        .http(request(&origin), grants(&origin, &["/search"]), false)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("read-only"));
    assert!(!format!("{error:#}").contains("private-query"));
    task.await.unwrap();
}

/// 【网络测试】【合法 POST 重定向】同一来源的已授权查询端点保留方法、认证头和正文。
#[tokio::test]
async fn authorized_post_redirects_preserve_the_request() {
    let (origin, task) = server(vec![redirect(307, "/second?step=2"), ok_response()]).await;
    let output = SaiPluginHost
        .http(
            request(&origin),
            grants(&origin, &["/search", "/second"]),
            false,
        )
        .await
        .unwrap();
    assert_eq!(output.text, "ok");
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /second?step=2 HTTP/1.1"));
    assert!(requests[1].contains("Bearer header-secret"));
    assert!(requests[1].ends_with("{\"query\":\"Rust\"}"));
}

/// 【网络测试】【方法切换】303 跳转改为 GET，移除原查询正文与内容类型。
#[tokio::test]
async fn see_other_redirects_convert_queries_to_bodyless_get_requests() {
    let (origin, task) = server(vec![redirect(303, "/result"), ok_response()]).await;
    SaiPluginHost
        .http(request(&origin), grants(&origin, &["/search"]), false)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    assert!(requests[1].starts_with("GET /result HTTP/1.1"));
    assert!(!requests[1].contains("application/json"));
    assert!(!requests[1].contains("Rust"));
}

/// 【网络测试】【跨来源认证】允许的 GET 跳转不会转发标准认证头或自定义密钥头。
#[tokio::test]
async fn cross_origin_redirects_strip_all_custom_credentials() {
    let (target, target_task) = server(vec![ok_response()]).await;
    let (origin, source_task) = server(vec![redirect(302, &format!("{target}/result"))]).await;
    let mut request = request(&origin);
    request.method = "GET".into();
    request.body = None;
    request
        .headers
        .insert("x-search-key".into(), "other-secret".into());
    request.headers.insert("accept".into(), "text/plain".into());
    let mut capabilities = grants(&origin, &[]);
    capabilities.http.insert(target);
    SaiPluginHost
        .http(request, capabilities, false)
        .await
        .unwrap();
    let first = source_task.await.unwrap();
    let last = target_task.await.unwrap();
    assert!(first[0].contains("header-secret"));
    assert!(!last[0].contains("secret"));
    assert!(last[0].contains("accept: text/plain"));
}

/// 【网络测试】【跨来源正文】307 保留正文时拒绝跨来源转发，查询端点授权不能泄露正文凭据。
#[tokio::test]
async fn cross_origin_post_redirects_do_not_forward_request_bodies() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_origin = format!("http://{}", target.local_addr().unwrap());
    let (origin, task) = server(vec![redirect(307, &format!("{target_origin}/search"))]).await;
    let mut capabilities = grants(&origin, &["/search"]);
    capabilities.http.insert(target_origin.clone());
    capabilities
        .http_read_only_post
        .insert(format!("{target_origin}/search"));
    assert!(SaiPluginHost
        .http(request(&origin), capabilities, false)
        .await
        .is_err());
    task.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), target.accept())
            .await
            .is_err()
    );
}

/// 【网络测试】【循环上限】HTTP 重定向最多跟随五次，循环不会无限占用请求。
#[tokio::test]
async fn redirect_loops_stop_after_five_followed_redirects() {
    let (origin, task) = server(vec![redirect(307, "/search"); 6]).await;
    let error = SaiPluginHost
        .http(request(&origin), grants(&origin, &["/search"]), false)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("redirect limit"));
    assert_eq!(task.await.unwrap().len(), 6);
}

/// 【网络测试】【跳转地址上限】重定向不能绕过初始请求的 URL 字节限制。
#[tokio::test]
async fn redirect_targets_obey_the_request_url_size_limit() {
    let target = format!("/search?value={}", "x".repeat(8192));
    let (origin, task) = server(vec![redirect(307, &target)]).await;
    let error = SaiPluginHost
        .http(request(&origin), grants(&origin, &["/search"]), false)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("URL exceeds size limit"));
    assert_eq!(task.await.unwrap().len(), 1);
}
