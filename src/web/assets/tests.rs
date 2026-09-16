use super::{serve, WebAssets};
use axum::Router;
use flate2::read::GzDecoder;
use reqwest::{Client, StatusCode};
use std::io::Read;

struct AssetServer {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for AssetServer {
    /// 【静态资源】【测试清理】终止当前测试的本地服务；无额外参数，无返回值。
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 【静态资源】【测试服务】启动真实资源处理器；无参数，返回服务地址与清理句柄。
async fn start_server() -> AssetServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(serve))
            .await
            .unwrap();
    });
    AssetServer { url, task }
}

/// 【静态资源】【测试样本】选择构建产物中的可压缩脚本；无参数，返回资源路径。
fn script_path() -> String {
    WebAssets::iter()
        .find(|path| {
            path.starts_with("assets/")
                && path.ends_with(".js")
                && WebAssets::get(path).is_some_and(|asset| asset.data.len() > 4096)
        })
        .expect("build web assets before testing the static server")
        .into_owned()
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
    assert_eq!(decoded, WebAssets::get(&path).unwrap().data.as_ref());
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
        WebAssets::get(&path).unwrap().data.as_ref()
    );
}
