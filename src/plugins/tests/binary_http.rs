use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::{
    host::{HttpRequest, PluginHost},
    Capabilities,
};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【二进制网络测试】【固定服务】返回单个原始响应并记录完整请求头。
/// @param response HTTP 响应字节
/// @returns 本地来源与记录任务
async fn server(response: Vec<u8>) -> (String, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let count = stream.read(&mut buffer).await.unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        stream.write_all(&response).await.unwrap();
        String::from_utf8(bytes).unwrap()
    });
    (origin, task)
}

/// 【二进制网络测试】【请求】构造匿名 GET，默认两秒内结束。
/// @param url 地址；max_bytes 为响应限制
/// @returns 二进制请求
fn request(url: String, max_bytes: usize) -> HttpRequest {
    HttpRequest {
        url,
        method: "GET".into(),
        headers: Default::default(),
        body: None,
        max_bytes,
        timeout_ms: 2000,
    }
}

/// 【二进制网络测试】【无文本解码】二进制接口忽略 charset，保留图片中的任意字节。
#[tokio::test]
async fn binary_http_preserves_raw_bytes_without_text_decoding() {
    let response=b"HTTP/1.1 200 OK\r\nContent-Type: image/png; charset=windows-1252\r\nContent-Length: 4\r\nConnection: close\r\n\r\n\xff\x00\x89\xe9".to_vec();
    let (origin, task) = server(response).await;
    let caps = Capabilities {
        http: [origin.clone()].into(),
        ..Default::default()
    };
    let response = SaiPluginHost
        .http_binary(request(origin, 4), caps, false)
        .await
        .unwrap();
    assert_eq!(response.body, vec![0xff, 0, 0x89, 0xe9]);
    assert_eq!(response.status, 200);
    task.await.unwrap();
}

/// 【二进制网络测试】【传输上限】声明长度与分块响应都受实际字节数限制。
#[tokio::test]
async fn declared_and_chunked_body_sizes_are_bounded() {
    for response in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n1234".to_vec(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\n12\r\n2\r\n34\r\n0\r\n\r\n".to_vec(),
    ] {
        let (origin,task)=server(response).await;
        let caps=Capabilities { http:[origin.clone()].into(),..Default::default() };
        let error=SaiPluginHost.download_binary(request(origin,3),caps).await.err().unwrap();
        assert!(format!("{error:#}").contains("byte limit")); task.await.unwrap();
    }
}

/// 【二进制网络测试】【私有地址】匿名公开授权不能访问回环、私有、映射及非 HTTP 地址。
#[tokio::test]
async fn public_downloads_reject_non_public_destinations_and_credentials() {
    let caps: Capabilities =
        serde_json::from_value(json!({"binary":{"public_downloads":true}})).unwrap();
    for url in [
        "http://127.0.0.1",
        "http://127.1",
        "http://localhost",
        "http://[::1]",
        "http://[::ffff:127.0.0.1]",
        "http://169.254.169.254",
        "http://10.0.0.1",
        "file:///etc/passwd",
        "https://user:secret@public.test/image?token=secret",
    ] {
        let error = SaiPluginHost
            .download_binary(request(url.into(), 1024), caps.clone())
            .await
            .err()
            .unwrap();
        let text = format!("{error:#}");
        assert!(!text.contains("secret"), "{text}");
        assert!(
            text.contains("non-public") || text.contains("HTTP(S)"),
            "{url}: {text}"
        );
    }
}

/// 【二进制网络测试】【逐跳检查】来源授权不能经重定向开放另一台未授权本地服务。
#[tokio::test]
async fn redirects_revalidate_private_destinations_before_connecting() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let response=format!("HTTP/1.1 302 Found\r\nLocation: http://{}/image?token=secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.local_addr().unwrap());
    let (origin, task) = server(response.into_bytes()).await;
    let caps: Capabilities =
        serde_json::from_value(json!({"http":[origin],"binary":{"public_downloads":true}}))
            .unwrap();
    let error = SaiPluginHost
        .download_binary(request(origin, 1024), caps)
        .await
        .err()
        .unwrap();
    let text = format!("{error:#}");
    assert!(text.contains("non-public"));
    assert!(!text.contains("secret"));
    task.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), target.accept())
            .await
            .is_err()
    );
}

/// 【二进制网络测试】【精确来源】两个本地来源都明确授权时允许跳转，Cookie 不进入后续请求。
#[tokio::test]
async fn explicitly_granted_redirects_remain_anonymous() {
    let (target, target_task) =
        server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_vec())
            .await;
    let response=format!("HTTP/1.1 302 Found\r\nLocation: {target}/image\r\nSet-Cookie: secret=fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let (origin, source_task) = server(response.into_bytes()).await;
    let caps = Capabilities {
        http: [origin.clone(), target].into(),
        ..Default::default()
    };
    let response = SaiPluginHost
        .download_binary(request(origin, 1024), caps)
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    source_task.await.unwrap();
    let request = target_task.await.unwrap().to_ascii_lowercase();
    assert!(request.starts_with("get /image "));
    assert!(!request.contains("cookie"));
    assert!(!request.contains("authorization"));
}
