use super::{serve, WebAssets};
use axum::Router;
use flate2::read::GzDecoder;
use reqwest::{Client, StatusCode};
use std::io::{Read, Write};

const SCRIPT: &str = "assets/index-1234abcd.js";

struct AssetServer {
    url: String,
    task: tokio::task::JoinHandle<()>,
    directory: tempfile::TempDir,
}

impl Drop for AssetServer {
    /// 【静态资源】【测试清理】终止当前测试的本地服务；无额外参数，无返回值。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【静态资源】【测试服务】启动真实资源处理器；无参数，返回服务地址与清理句柄。
async fn start_server() -> AssetServer {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("assets")).unwrap();
    std::fs::write(
        directory.path().join("index.html"),
        "<html><body>Sai fixture</body></html>",
    )
    .unwrap();
    let script = "console.info('fixture');\n".repeat(300);
    std::fs::write(directory.path().join(SCRIPT), &script).unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(script.as_bytes()).unwrap();
    std::fs::write(
        directory.path().join(format!("{SCRIPT}.gz")),
        gzip.finish().unwrap(),
    )
    .unwrap();
    let assets = WebAssets::new(directory.path().to_path_buf()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(serve).layer(axum::Extension(assets)),
        )
        .await
        .unwrap();
    });
    AssetServer {
        url,
        task,
        directory,
    }
}

/// 【静态资源】【测试样本】使用独立测试脚本；无参数，返回资源路径。
fn script_path() -> String {
    SCRIPT.to_string()
}

/// 【静态资源】【条件请求】哈希资源使用长期缓存并支持实体标签校验；无参数，无返回值。
#[tokio::test]
async fn hashed_assets_support_immutable_cache_and_revalidation() {
    let server = start_server().await;
    let url = format!("{}/{}", server.url, script_path());
    let client = Client::new();
    let first = client.get(&url).send().await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        first.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );
    let etag = first.headers().get("etag").unwrap().clone();
    let cached = client
        .get(&url)
        .header("if-none-match", etag)
        .send()
        .await
        .unwrap();
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    assert!(cached.bytes().await.unwrap().is_empty());
}

/// 【静态资源】【入口缓存】路由回退仍使用入口页面的校验策略；无参数，无返回值。
#[tokio::test]
async fn route_fallback_revalidates_html_without_immutable_cache() {
    let server = start_server().await;
    let response = Client::new()
        .get(format!("{}/settings/providers", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-cache");
    assert!(response.headers().get("etag").is_some());
    assert!(response.text().await.unwrap().contains("<html"));
}

/// 【静态资源】【压缩响应】协商 gzip 后仍能还原完整脚本；无参数，无返回值。
#[tokio::test]
async fn gzip_response_decodes_to_the_original_asset() {
    let server = start_server().await;
    let path = script_path();
    let response = Client::new()
        .get(format!("{}/{path}", server.url))
        .header("accept-encoding", "gzip")
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers().get("content-encoding").unwrap(), "gzip");
    assert_eq!(response.headers().get("vary").unwrap(), "Accept-Encoding");
    let encoded = response.bytes().await.unwrap();
    let mut decoded = Vec::new();
    GzDecoder::new(encoded.as_ref())
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(
        decoded,
        std::fs::read(server.directory.path().join(&path)).unwrap()
    );
    assert!(encoded.len() < decoded.len());
}

/// 【静态资源】【压缩协商】明确拒绝 gzip 时返回原始表示；无参数，无返回值。
#[tokio::test]
async fn explicit_gzip_rejection_overrides_wildcard_acceptance() {
    let server = start_server().await;
    let path = script_path();
    let response = Client::new()
        .get(format!("{}/{path}", server.url))
        .header("accept-encoding", "gzip;q=0, *;q=1")
        .send()
        .await
        .unwrap();
    assert!(response.headers().get("content-encoding").is_none());
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        std::fs::read(server.directory.path().join(&path)).unwrap()
    );
}

/// 【Web 资源】【请求边界】不存在的资源不回退到 HTML，HEAD 与方法限制保持正确
/// 参数: 无；返回: 无
#[tokio::test]
async fn missing_files_head_and_unsupported_methods() {
    let server = start_server().await;
    let client = Client::new();
    let response = client
        .get(format!("{}/assets/missing.js", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = client
        .head(format!("{}/index.html", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("content-length").is_some());
    assert!(response.bytes().await.unwrap().is_empty());
    let response = client.post(&server.url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    for path in [
        "../secret",
        "/secret",
        "C:/secret",
        "dir\\secret",
        "a/../../secret",
    ] {
        assert!(!super::safe_relative_path(path));
    }
}

/// 【Web 资源】【符号链接边界】指向资源目录外的文件不可通过静态服务读取
/// 参数: 无；返回: 无
#[cfg(unix)]
#[tokio::test]
async fn symlinks_cannot_expose_files_outside_the_bundle() {
    let server = start_server().await;
    let secret = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(secret.path(), "secret").unwrap();
    std::os::unix::fs::symlink(secret.path(), server.directory.path().join("secret.txt")).unwrap();
    let response = Client::new()
        .get(format!("{}/secret.txt", server.url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
