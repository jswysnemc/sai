use super::policy;
use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

/// 【网络授权】【请求代理】限制头部、域名和端口，固定 DNS 地址后转发一次连接。
/// 参数：来自隔离进程的流及域名规则；返回转发结果。
pub(super) async fn serve<S>(mut stream: S, domains: &[String]) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let result = forward(&mut stream, domains).await;
    if result.is_err() {
        let _ = stream
            .write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
            .await;
    }
    result
}

/// 【网络授权】【连接路由】解析 HTTP 首部后建立单一上游，CONNECT 后也不能重新选择目标。
/// 参数：客户端流与域名规则；返回转发结果。
async fn forward<S>(stream: &mut S, domains: &[String]) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut header = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !header.ends_with(b"\r\n\r\n") {
            anyhow::ensure!(header.len() < 16_384, "proxy header is too large");
            header.push(stream.read_u8().await?);
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    let text = std::str::from_utf8(&header).context("proxy header must be UTF-8")?;
    let (first, headers) = text
        .split_once("\r\n")
        .context("proxy request line missing")?;
    let parts = first.split_whitespace().collect::<Vec<_>>();
    anyhow::ensure!(
        parts.len() == 3 && matches!(parts[2], "HTTP/1.1" | "HTTP/1.0"),
        "invalid proxy request"
    );
    let tunnel = parts[0] == "CONNECT";
    let url = reqwest::Url::parse(&if tunnel {
        format!("https://{}/", parts[1])
    } else {
        parts[1].into()
    })?;
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
        "proxy target contains credentials or fragment"
    );
    anyhow::ensure!(
        if tunnel {
            url.scheme() == "https" && url.path() == "/" && url.query().is_none()
        } else {
            url.scheme() == "http"
        },
        "unsupported proxy scheme"
    );
    let host = url.host_str().context("proxy target host missing")?;
    let port = url
        .port_or_known_default()
        .context("proxy target port missing")?;
    let addresses = policy::resolve(host, port, domains).await?;
    let mut upstream = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        TcpStream::connect(addresses.as_slice()),
    )
    .await??;
    if tunnel {
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
    } else {
        let path = format!(
            "{}{}",
            url.path(),
            url.query()
                .map(|query| format!("?{query}"))
                .unwrap_or_default()
        );
        let mut request = format!("{} {path} {}\r\n", parts[0], parts[2]);
        for line in headers.split("\r\n").filter(|line| !line.is_empty()) {
            let (name, _) = line.split_once(':').context("invalid proxy header")?;
            if !matches!(
                name.to_ascii_lowercase().as_str(),
                "proxy-authorization" | "proxy-connection" | "connection" | "host"
            ) {
                request.push_str(line);
                request.push_str("\r\n");
            }
        }
        request.push_str(&format!("Host: {host}:{port}\r\nConnection: close\r\n\r\n"));
        upstream.write_all(request.as_bytes()).await?;
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::io::copy_bidirectional(stream, &mut upstream),
    )
    .await??;
    Ok(())
}
