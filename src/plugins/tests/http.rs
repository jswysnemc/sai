use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{HttpRequest, PluginHost};
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【插件测试】【HTTP 端点】启动只接受一次请求的本地端点。
/// @param response 完整 HTTP 响应字节
/// @returns 精确来源和可等待的服务任务
async fn server(response: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        stream.read(&mut bytes).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    (origin, task)
}

/// 【插件测试】【HTTP 请求】构造不含可选头和正文的测试请求。
/// @param url 请求地址；max_bytes 为响应字节上限
/// @returns GET 请求
fn request(url: String, max_bytes: usize) -> HttpRequest {
    HttpRequest {
        url,
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        max_bytes,
        timeout_ms: 30_000,
    }
}

/// 【插件测试】【重定向授权】宿主在跟随重定向之前检查目标来源，错误不包含查询凭据。
#[tokio::test]
async fn redirects_cannot_escape_the_granted_origin() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let response = format!("HTTP/1.1 302 Found\r\nLocation: http://{}/secret?token=private-value\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", target.local_addr().unwrap());
    let (origin, task) = server(response.into_bytes()).await;
    let error = SaiPluginHost
        .http(request(format!("{origin}/"), 1024), vec![origin])
        .await
        .unwrap_err();
    task.await.unwrap();
    assert!(!format!("{error:#}").contains("private-value"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), target.accept())
            .await
            .is_err()
    );
}

/// 【插件测试】【文本解码】宿主尊重 charset，且解码后的 UTF-8 文本同样受大小限制。
#[tokio::test]
async fn http_decoding_and_response_limits_apply_to_real_responses() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=windows-1252\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\xe9".to_vec();
    let (origin, task) = server(response.clone()).await;
    let output = SaiPluginHost
        .http(request(format!("{origin}/"), 8), vec![origin])
        .await
        .unwrap();
    task.await.unwrap();
    assert_eq!(output.text, "é");
    let (origin, task) = server(response).await;
    assert!(SaiPluginHost
        .http(request(format!("{origin}/"), 1), vec![origin])
        .await
        .is_err());
    task.await.unwrap();
    let (origin, task) =
        server(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n123456".to_vec())
            .await;
    assert!(SaiPluginHost
        .http(request(format!("{origin}/"), 3), vec![origin])
        .await
        .is_err());
    task.await.unwrap();
}
