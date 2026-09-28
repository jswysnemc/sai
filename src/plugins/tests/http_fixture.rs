use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// 【插件测试】【请求读取】完整读取本地 GET 请求头，避免部分读取后关闭连接。
/// @param stream 本地测试连接
/// @returns 无；超时、请求头过大或连接提前关闭时测试失败
pub(super) async fn read_request_headers(stream: &mut TcpStream) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            assert!(
                header.len() < 16 * 1024,
                "request headers exceed fixture limit"
            );
            stream.read_exact(&mut byte).await.unwrap();
            header.push(byte[0]);
        }
    })
    .await
    .expect("fixture request headers timed out");
}
