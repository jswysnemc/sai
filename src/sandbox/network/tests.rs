use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【网络授权】【真实代理回归】客户端提交未授权域名与本机地址时必须收到拒绝，不能建立隧道。
/// 参数：无；返回断言结果。
#[tokio::test]
async fn broker_rejects_unapproved_and_private_destinations() {
    for target in ["blocked.invalid:443", "127.0.0.1:443", "example.com:22"] {
        let (mut client, server) = tokio::io::duplex(4096);
        let target = target.to_string();
        let broker = tokio::spawn(async move {
            proxy::serve(server, &["example.com".into(), "127.0.0.1".into()]).await
        });
        client
            .write_all(format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut output = Vec::new();
        client.read_to_end(&mut output).await.unwrap();
        assert!(String::from_utf8_lossy(&output).starts_with("HTTP/1.1 403"));
        assert!(broker.await.unwrap().is_err());
    }
}
