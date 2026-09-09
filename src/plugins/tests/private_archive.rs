use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    host::{ArchiveRequest, FileReadRequest, PluginHost},
    Capabilities,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 【归档网络测试】【固定服务】只监听环回地址，发送预先构造的字节报文。
/// @param responses HTTP 响应列表
/// @returns 来源与有界服务任务
async fn server(responses: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        for response in responses {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut buffer = [0u8; 4096];
            stream.read(&mut buffer).await.unwrap();
            stream.write_all(&response).await.unwrap();
        }
    });
    (origin, task)
}

/// 【归档网络测试】【二进制报文】使用精确 Content-Length，正文可能不是 UTF-8。
/// @param bytes 原始响应正文
/// @returns HTTP 报文
fn response(bytes: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .into_bytes();
    response.extend_from_slice(bytes);
    response
}

/// 【归档网络测试】【请求】提供小型固定下载预算。
/// @param origin 本地服务来源
/// @returns 归档请求
fn request(origin: &str) -> ArchiveRequest {
    ArchiveRequest {
        url: format!("{origin}/archive"),
        destination: "snapshot".into(),
        max_bytes: 65536,
        max_unpacked_bytes: 65536,
        max_entries: 16,
        timeout_ms: 2000,
    }
}

/// 【归档网络测试】【授权】仅授予当前固定来源和私有工作目录。
/// @param origin 本地服务来源
/// @returns 有效能力
fn grants(origin: &str) -> Capabilities {
    let mut caps = super::private_storage::capabilities();
    caps.http.insert(origin.to_string());
    caps
}

/// 【归档网络测试】【二进制与跳转】合法同来源跳转保留压缩字节，解压后通过正式目录接口读取。
#[tokio::test]
async fn private_archive_binary_download_and_same_origin_redirect_work() {
    use std::io::Write;
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(b"pkgname=demo\n".len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "demo/PKGBUILD", b"pkgname=demo\n".as_slice())
        .unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&tar.into_inner().unwrap()).unwrap();
    let bytes = gzip.finish().unwrap();
    let redirect = b"HTTP/1.1 302 Found\r\nLocation: /second\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    let (origin, task) = server(vec![redirect, response(&bytes)]).await;
    let root = tempfile::tempdir().unwrap();
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "archive");
    let caps = grants(&origin);
    let work = host.workspace("download", "session", &caps).unwrap();
    work.extract_archive(request(&origin), caps).await.unwrap();
    let text = work
        .read_text(FileReadRequest {
            path: "snapshot/demo/PKGBUILD".into(),
            max_bytes: 64,
            lossy: false,
        })
        .await
        .unwrap();
    assert_eq!(text.text, "pkgname=demo\n");
    task.await.unwrap();
}

/// 【归档网络测试】【逐跳授权】未授权跳转不能触达目标服务，也不留下解压目录。
#[tokio::test]
async fn private_archive_redirect_cannot_add_an_http_origin() {
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let location = format!("http://{}/outside", target.local_addr().unwrap());
    let redirect = format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let (origin, task) = server(vec![redirect.into_bytes()]).await;
    let root = tempfile::tempdir().unwrap();
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "archive");
    let caps = grants(&origin);
    let work = host.workspace("download", "session", &caps).unwrap();
    assert!(format!(
        "{:#}",
        work.extract_archive(request(&origin), caps)
            .await
            .unwrap_err()
    )
    .contains("origin is not allowed"));
    assert!(
        tokio::time::timeout(Duration::from_millis(80), target.accept())
            .await
            .is_err()
    );
    assert_eq!(
        work.read_directory(".".into(), 16)
            .await
            .unwrap()
            .entries
            .len(),
        0
    );
    task.await.unwrap();
}

/// 【归档网络测试】【失败原子性】非法压缩正文和超出下载上限都不发布半成品。
#[tokio::test]
async fn private_archive_failed_downloads_leave_no_partial_directory() {
    for maximum in [4, 65536] {
        let (origin, task) = server(vec![response(b"not a tar.gz archive")]).await;
        let root = tempfile::tempdir().unwrap();
        let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "archive");
        let caps = grants(&origin);
        let work = host.workspace("download", "session", &caps).unwrap();
        let mut request = request(&origin);
        request.max_bytes = maximum;
        assert!(work.extract_archive(request, caps).await.is_err());
        assert!(work
            .read_directory(".".into(), 16)
            .await
            .unwrap()
            .entries
            .is_empty());
        task.await.unwrap();
    }
}
