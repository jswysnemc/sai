//! 下载官方 cua-driver 发布包并安装到 Sai 数据目录。

use super::archive::extract_driver;
use super::detect::find_driver_binary;
use super::release::{checksum_for, current_platform, release_urls};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_DOWNLOAD_BYTES: u64 = 80 * 1024 * 1024;

/// 安装结果。
#[derive(Debug, Serialize)]
pub struct InstallResult {
    /// 已安装驱动的绝对路径
    pub path: String,
    /// 安装的版本
    pub version: String,
    /// 当前平台
    pub platform: String,
}

/// 【桌面操作】【安装目录】返回 Sai 数据目录下的驱动安装根目录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 安装目录
pub(crate) fn install_dir(paths: &SaiPaths) -> PathBuf {
    paths.data_dir.join("computer-use")
}

/// 【桌面操作】【官方安装】下载固定版本并校验 SHA256 后落盘。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 安装结果
pub(crate) fn install(paths: &SaiPaths) -> Result<InstallResult> {
    install_with_fetcher(paths, download)
}

/// 【桌面操作】【安装编排】按发布地址下载、校验、解压并替换安装目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `fetch`: 按 URL 返回字节
///
/// 返回:
/// - 安装结果
pub(crate) fn install_with_fetcher(
    paths: &SaiPaths,
    fetch: impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<InstallResult> {
    let target = current_platform().ok_or_else(|| {
        anyhow::anyhow!(
            "automatic cua-driver install is unavailable on {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let urls = release_urls(&target.archive_name);
    // 1. 先取校验和，归档损坏时拒绝安装
    let sums =
        fetch(&urls.checksums).with_context(|| format!("failed to download {}", urls.checksums))?;
    let expected = checksum_for(&String::from_utf8_lossy(&sums), &target.archive_name)?;
    let archive =
        fetch(&urls.archive).with_context(|| format!("failed to download {}", urls.archive))?;
    let actual = hex::encode(Sha256::digest(&archive));
    if actual != expected {
        bail!(
            "archive checksum mismatch for {}: expected {expected}, got {actual}",
            target.archive_name
        );
    }
    // 2. 解压到同级临时目录后替换，避免半成品覆盖现有安装
    let destination = install_dir(paths);
    std::fs::create_dir_all(destination.parent().unwrap_or_else(|| Path::new(".")))?;
    let staging = destination
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".sai-cua-driver-{}", Uuid::new_v4().simple()));
    let extract_result = extract_driver(&archive, &target.archive_name, &staging);
    if let Err(error) = extract_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if destination.exists() {
        std::fs::remove_dir_all(&destination)
            .with_context(|| format!("failed to replace {}", destination.display()))?;
    }
    std::fs::rename(&staging, &destination).with_context(|| {
        format!(
            "failed to move {} to {}",
            staging.display(),
            destination.display()
        )
    })?;
    let binary = find_driver_binary(&destination)
        .ok_or_else(|| anyhow::anyhow!("installed archive is missing cua-driver"))?;
    Ok(InstallResult {
        path: binary.display().to_string(),
        version: super::PINNED_VERSION.to_string(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

/// 【桌面操作】【发布下载】下载官方发布文件，限制体积与超时。
///
/// 参数:
/// - `url`: 下载地址
///
/// 返回:
/// - 文件字节
fn download(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent("sai-computer-use/0.2.10")
        .build()?;
    let response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .with_context(|| format!("download failed: {url}"))?;
    if let Some(length) = response.content_length() {
        if length > MAX_DOWNLOAD_BYTES {
            bail!("{url} is larger than 80 MiB");
        }
    }
    let bytes = response.bytes().context("failed to read download body")?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        bail!("{url} is larger than 80 MiB");
    }
    Ok(bytes.to_vec())
}
