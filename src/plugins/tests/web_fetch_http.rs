use super::web_fetch_http_support::{response, server};
use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::{
    host::{HttpRequest, PluginHost},
    Capabilities,
};
use serde_json::json;

/// 【网页网络测试】【请求构造】使用公开序列化契约构造不同重定向和错误正文策略
/// @param origin 本地来源；redirects 为跟随次数；read_error_body 为是否读取错误正文
/// @returns 二进制请求
fn request(origin: &str, redirects: usize, read_error_body: bool) -> HttpRequest {
    serde_json::from_value(json!({"url":origin,"max_bytes":1024,"timeout_ms":2000,"max_redirects":redirects,"read_error_body":read_error_body})).unwrap()
}

/// 【网页网络测试】【十次跳转】显式请求可跟随十次重定向，默认上限仍由请求契约控制
/// @returns 无；第十一段目标不再连接
#[tokio::test]
async fn binary_requests_follow_ten_redirects_when_explicitly_requested() {
    for (redirects, success) in [(10, true), (11, false)] {
        let mut replies = vec![response(302, "Location: /next\r\n", b""); redirects.min(11)];
        if success {
            replies.push(response(200, "", b"ok"));
        }
        let (origin, task) = server(replies).await;
        let caps = Capabilities {
            http: [origin.clone()].into(),
            ..Default::default()
        };
        let result = SaiPluginHost
            .http_binary(request(&origin, 10, true), caps, false)
            .await;
        assert_eq!(result.is_ok(), success);
        if !success {
            assert!(format!("{:#}", result.err().unwrap()).contains("redirect limit"));
        }
        assert_eq!(task.await.unwrap().len(), 11);
    }
}

/// 【网页网络测试】【状态先行】错误状态可在读取超限或未完整到达的正文之前交给插件
/// @returns 无；错误响应保留状态和头部，成功响应仍严格执行字节上限
#[tokio::test]
async fn headers_only_error_policy_precedes_body_limits() {
    for (status, read_error_body, success) in [
        (404, false, true),
        (503, false, true),
        (404, true, false),
        (200, false, false),
    ] {
        let wire = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: 6291456\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let (origin, task) = server(vec![wire]).await;
        let caps = Capabilities {
            http: [origin.clone()].into(),
            ..Default::default()
        };
        let result = SaiPluginHost
            .http_binary(request(&origin, 5, read_error_body), caps, false)
            .await;
        assert_eq!(result.is_ok(), success);
        if let Ok(result) = result {
            assert_eq!(result.status, status);
            assert!(result.body.is_empty());
        }
        task.await.unwrap();
    }
}

/// 【网页网络测试】【宿主硬上限】直接调用宿主也不能超过十次重定向
/// @returns 无；非法请求在建立连接之前失败
#[tokio::test]
async fn redirect_hard_limit_is_enforced_without_the_lua_binding() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let caps = Capabilities {
        http: [origin.clone()].into(),
        ..Default::default()
    };
    for download in [true, false] {
        let result = if download {
            SaiPluginHost
                .download_binary(request(&origin, 11, true), caps.clone())
                .await
        } else {
            SaiPluginHost
                .http_binary(request(&origin, 11, true), caps.clone(), false)
                .await
        };
        assert!(format!("{:#}", result.err().unwrap()).contains("redirect limit"));
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}
