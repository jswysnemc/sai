use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【网页网络测试】【本地服务】按顺序提供原始响应，记录每次请求并允许客户端提前断开
/// @param responses 完整 HTTP 响应列表
/// @returns 仅绑定回环的来源和可等待的记录任务
pub(super) async fn server(
    responses: Vec<Vec<u8>>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let count = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
                    .await
                    .unwrap()
                    .unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(bytes).unwrap());
            let _ = stream.write_all(&response).await;
        }
        requests
    });
    (origin, task)
}

/// 【网页网络测试】【响应构造】构造带明确长度并关闭连接的固定原始响应
/// @param status HTTP 状态；headers 为额外头部；body 为原始字节
/// @returns 完整可发送响应
pub(super) fn response(status: u16, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 {status} Fixture\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}
