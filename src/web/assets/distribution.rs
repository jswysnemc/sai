use super::installation;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

const REPOSITORY: &str = "https://github.com/jswysnemc/sai/releases/download";
pub(super) const ARCHIVE: &str = "sai-web.tar.gz";
const MAX_DOWNLOAD_BYTES: usize = 128 * 1024 * 1024;

/// 【Web 分发】【启动准备】首次启动下载同版本资源，已有缓存时离线复用
/// 参数: paths 为应用路径，update 为是否强制更新；返回: 可供服务使用的资源目录
pub(crate) async fn ensure(paths: &SaiPaths, update: bool) -> Result<PathBuf> {
    if let Some(directory) = std::env::var_os("SAI_WEB_ASSETS_DIR") {
        if update {
            bail!("SAI_WEB_ASSETS_DIR is set; unset it before running sai web update");
        }
        let directory = PathBuf::from(directory)
            .canonicalize()
            .context("resolve SAI_WEB_ASSETS_DIR")?;
        anyhow::ensure!(
            directory.join("index.html").is_file(),
            "SAI_WEB_ASSETS_DIR has no index.html; build the frontend first"
        );
        return Ok(directory);
    }
    let version = env!("CARGO_PKG_VERSION");
    let root = paths.data_dir.join("web").join(format!("v{version}"));
    ensure_from_release(&root, version, &format!("{REPOSITORY}/v{version}"), update)
        .await
        .with_context(|| format!("Unable to prepare Sai Web v{version}. Check GitHub access and that this release contains {ARCHIVE}; retry with `sai web update`, or set SAI_WEB_ASSETS_DIR to a local frontend build"))
}

/// 【Web 分发】【版本获取】下载并校验发布包，完整安装后才切换当前版本指针
/// 参数: root 为版本缓存，version 为后端版本，release 为发布地址，update 控制刷新；返回: 资源目录
pub(super) async fn ensure_from_release(
    root: &Path,
    version: &str,
    release: &str,
    update: bool,
) -> Result<PathBuf> {
    if !update {
        if let Some(directory) = installation::current(root, version) {
            return Ok(directory);
        }
    }
    eprintln!("【Web 分发】【下载资源】Sai Web v{version}");
    let client = reqwest::Client::builder()
        .user_agent(concat!("sai/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(180))
        .build()?;
    let checksum = download(&client, &format!("{release}/{ARCHIVE}.sha256"), 1024).await?;
    let checksum = std::str::from_utf8(&checksum)?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    installation::validate_digest(&checksum)?;
    let archive = download(&client, &format!("{release}/{ARCHIVE}"), MAX_DOWNLOAD_BYTES).await?;
    let root = root.to_path_buf();
    let version = version.to_string();
    tokio::task::spawn_blocking(move || installation::install(&root, &version, &archive, &checksum))
        .await
        .context("join Web asset installation")?
}

/// 【Web 分发】【有界下载】检查 HTTP 状态及实际字节数，避免错误页面或无限响应进入安装流程
/// 参数: client 为客户端，url 为资源地址，limit 为字节上限；返回: 下载内容或错误
async fn download(client: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    anyhow::ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= limit as u64),
        "Web download exceeds size limit"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            chunk.len() <= limit.saturating_sub(bytes.len()),
            "Web download exceeds size limit"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
