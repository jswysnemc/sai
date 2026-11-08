use super::{distribution, installation};
use axum::{routing::get, Router};
use flate2::{write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// 【Web 分发】【测试归档】从文件列表创建真实压缩包，支持注入异常条目
/// 参数: entries 为路径、类型与内容列表；返回: tar.gz 字节
fn archive(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (path, kind, content) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        header.set_cksum();
        builder.append(&header, *content).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// 【Web 分发】【版本样本】创建带版本清单、入口与脚本的测试包
/// 参数: version 为版本，html 为入口内容；返回: 压缩包及摘要
fn bundle(version: &str, html: &str) -> (Vec<u8>, String) {
    let manifest = serde_json::json!({"version": version}).to_string();
    let bytes = archive(&[
        ("sai-web.json", tar::EntryType::Regular, manifest.as_bytes()),
        ("index.html", tar::EntryType::Regular, html.as_bytes()),
        (
            "assets/index-1234abcd.js",
            tar::EntryType::Regular,
            b"export {}",
        ),
    ]);
    let digest = hex::encode(Sha256::digest(&bytes));
    (bytes, digest)
}

/// 【Web 分发】【下载安装回归】实际 HTTP 下载后离线复用，同版本刷新切换目录且保留旧文件
/// 参数: 无；返回: 无
#[tokio::test]
async fn first_download_offline_reuse_and_atomic_update() {
    let root = tempfile::tempdir().unwrap();
    let (bytes, digest) = bundle("1.2.3", "<html>first</html>");
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let router = Router::new()
        .route(
            "/v1.2.3/sai-web.tar.gz",
            get(move || {
                let bytes = bytes.clone();
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    bytes
                }
            }),
        )
        .route(
            "/v1.2.3/sai-web.tar.gz.sha256",
            get(move || {
                let digest = digest.clone();
                async move { format!("{digest}  sai-web.tar.gz\n") }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1.2.3", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (first, second) = tokio::join!(
        distribution::ensure_from_release(root.path(), "1.2.3", &url, false),
        distribution::ensure_from_release(root.path(), "1.2.3", &url, false),
    );
    let first = first.unwrap();
    assert!(second.unwrap().join("index.html").is_file());
    assert!(requests.load(Ordering::SeqCst) >= 1);
    std::fs::remove_file(first.join("assets/index-1234abcd.js")).unwrap();
    let refreshed = distribution::ensure_from_release(root.path(), "1.2.3", &url, true)
        .await
        .unwrap();
    assert_ne!(first, refreshed);
    assert!(refreshed.join("assets/index-1234abcd.js").is_file());
    server.abort();
    let _ = server.await;
    let cached = distribution::ensure_from_release(root.path(), "1.2.3", &url, false)
        .await
        .unwrap();
    assert_eq!(cached, refreshed);
    assert!(
        distribution::ensure_from_release(root.path(), "1.2.3", &url, true)
            .await
            .is_err()
    );
    assert_eq!(
        installation::current(root.path(), "1.2.3"),
        Some(refreshed.clone())
    );
    let (bytes, digest) = bundle("1.2.3", "<html>updated</html>");
    let updated = installation::install(root.path(), "1.2.3", &bytes, &digest).unwrap();
    assert_ne!(first, updated);
    assert_eq!(
        std::fs::read_to_string(first.join("index.html")).unwrap(),
        "<html>first</html>"
    );
    assert_eq!(installation::current(root.path(), "1.2.3"), Some(updated));
}

/// 【Web 分发】【失败回滚回归】摘要、版本、结构、路径与链接错误均不替换可用缓存
/// 参数: 无；返回: 无
#[test]
fn invalid_archives_preserve_the_current_bundle() {
    let root = tempfile::tempdir().unwrap();
    let (bytes, digest) = bundle("1.2.3", "<html>valid</html>");
    let current = installation::install(root.path(), "1.2.3", &bytes, &digest).unwrap();
    assert!(installation::install(root.path(), "1.2.3", b"corrupt", &digest).is_err());
    let (wrong_version, wrong_digest) = bundle("9.9.9", "wrong version");
    assert!(installation::install(root.path(), "1.2.3", &wrong_version, &wrong_digest).is_err());
    for invalid in [
        archive(&[("index.html", tar::EntryType::Regular, b"missing manifest")]),
        archive(&[("../outside", tar::EntryType::Regular, b"escape")]),
        archive(&[("/absolute", tar::EntryType::Regular, b"escape")]),
        archive(&[("linked", tar::EntryType::Symlink, b"")]),
        archive(&[("linked", tar::EntryType::Link, b"")]),
        bytes[..bytes.len() - 8].to_vec(),
    ] {
        let checksum = hex::encode(Sha256::digest(&invalid));
        assert!(installation::install(root.path(), "1.2.3", &invalid, &checksum).is_err());
        assert_eq!(
            installation::current(root.path(), "1.2.3"),
            Some(current.clone())
        );
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
}

/// 【Web 分发】【CLI 回归】update 作为子命令解析，不与服务启动参数混淆
/// 参数: 无；返回: 无
#[test]
fn update_is_an_optional_web_subcommand() {
    use clap::Parser;
    let cli = crate::cli::Cli::try_parse_from(["sai", "web", "update"]).unwrap();
    assert!(matches!(cli.command, Some(crate::cli::Command::Web(args)) if args.command.is_some()));
    let cli = crate::cli::Cli::try_parse_from(["sai", "web", "--port", "5000"]).unwrap();
    assert!(
        matches!(cli.command, Some(crate::cli::Command::Web(args)) if args.command.is_none() && args.port == 5000)
    );
    assert!(crate::cli::Cli::try_parse_from(["sai", "web", "unknown"]).is_err());
}

/// 【Web 分发】【发布包验收】验证真实打包脚本产物能被安装并保持资源内容
/// 参数: 无；返回: 无；需先运行 node web/scripts/package-release.mjs
#[test]
#[ignore = "requires an explicitly packaged frontend release"]
fn packaged_release_installs_successfully() {
    let package =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web-release/sai-web.tar.gz");
    let bytes = std::fs::read(&package).unwrap();
    let checksum = std::fs::read_to_string(package.with_extension("gz.sha256")).unwrap();
    let root = tempfile::tempdir().unwrap();
    let directory = installation::install(
        root.path(),
        env!("CARGO_PKG_VERSION"),
        &bytes,
        checksum.split_whitespace().next().unwrap(),
    )
    .unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist/index.html");
    assert_eq!(
        std::fs::read(directory.join("index.html")).unwrap(),
        std::fs::read(source).unwrap()
    );
}
