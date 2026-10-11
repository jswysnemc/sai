//! 官方 cua-driver 发布资源名与校验和解析。

use anyhow::{bail, Result};

/// 文档要求的最低版本。
pub(crate) const MIN_VERSION: &str = "0.34.1";
/// 安装接口固定下载的版本。
pub(crate) const PINNED_VERSION: &str = "0.34.1";
const REPO: &str = "trycua/cua";
const TAG_PREFIX: &str = "cua-driver-rs-v";

/// 当前平台对应的发布资源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlatformTarget {
    /// 资源标签，例如 linux-x86_64
    pub label: &'static str,
    /// 归档文件名
    pub archive_name: String,
}

/// 【桌面操作】【平台映射】按当前操作系统和架构选择官方发布资源。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 支持自动安装时返回资源描述
pub(crate) fn current_platform() -> Option<PlatformTarget> {
    platform_target(std::env::consts::OS, std::env::consts::ARCH)
}

/// 【桌面操作】【资源选择】把操作系统和架构映射到官方归档名。
///
/// 参数:
/// - `os`: 操作系统
/// - `arch`: 架构
///
/// 返回:
/// - 支持时返回资源描述
pub(crate) fn platform_target(os: &str, arch: &str) -> Option<PlatformTarget> {
    let (label, archive_name) = match (os, arch) {
        ("linux", "x86_64") => (
            "linux-x86_64",
            format!("cua-driver-rs-{PINNED_VERSION}-linux-x86_64-binary.tar.gz"),
        ),
        ("linux", "aarch64") => (
            "linux-arm64",
            format!("cua-driver-rs-{PINNED_VERSION}-linux-arm64-binary.tar.gz"),
        ),
        ("macos", "x86_64" | "aarch64") => (
            "darwin-universal",
            format!("cua-driver-rs-{PINNED_VERSION}-darwin-universal.tar.gz"),
        ),
        ("windows", "x86_64") => (
            "windows-x86_64",
            format!("cua-driver-rs-{PINNED_VERSION}-windows-x86_64.zip"),
        ),
        ("windows", "aarch64") => (
            "windows-arm64",
            format!("cua-driver-rs-{PINNED_VERSION}-windows-arm64.zip"),
        ),
        _ => return None,
    };
    Some(PlatformTarget {
        label,
        archive_name,
    })
}

/// 官方发布下载地址。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseUrls {
    /// 归档地址
    pub archive: String,
    /// SHA256SUMS 地址
    pub checksums: String,
}

/// 【桌面操作】【发布地址】构造固定版本的 GitHub Releases 下载地址。
///
/// 参数:
/// - `archive_name`: 归档文件名
///
/// 返回:
/// - 归档与校验和地址
pub(crate) fn release_urls(archive_name: &str) -> ReleaseUrls {
    let tag = format!("{TAG_PREFIX}{PINNED_VERSION}");
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    ReleaseUrls {
        archive: format!("{base}/{archive_name}"),
        checksums: format!("{base}/SHA256SUMS"),
    }
}

/// 【桌面操作】【校验和】从 SHA256SUMS 中读取指定归档的十六进制摘要。
///
/// 参数:
/// - `text`: SHA256SUMS 文本
/// - `archive_name`: 归档文件名
///
/// 返回:
/// - 小写十六进制摘要
pub(crate) fn checksum_for(text: &str, archive_name: &str) -> Result<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        let name = name.trim_start_matches('*');
        if name == archive_name {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    bail!("SHA256SUMS does not list {archive_name}");
}
