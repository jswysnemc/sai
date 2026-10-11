//! 本机 cua-driver 探测与按需安装，不自动开启桌面操作或授予系统权限。

mod archive;
mod detect;
mod install;
mod process;
mod release;

#[cfg(test)]
mod tests;

use crate::paths::SaiPaths;
use anyhow::Result;
use serde::Serialize;

pub(crate) use install::{install, InstallResult};
pub(crate) use process::version_at_least;
pub(crate) use release::{MIN_VERSION, PINNED_VERSION};

/// 本机 cua-driver 探测结果。
#[derive(Debug, Serialize)]
pub struct DriverStatus {
    /// 当前操作系统
    pub platform: String,
    /// 当前架构
    pub arch: String,
    /// 配置中的驱动路径或文件名
    pub configured_path: String,
    /// 实际解析到的路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<String>,
    /// 解析路径是否指向已存在文件
    pub installed: bool,
    /// 探测到的版本号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 版本是否达到文档要求的最低版本
    pub version_ok: bool,
    /// 文档要求的最低版本
    pub min_version: &'static str,
    /// 安装接口固定下载的版本
    pub pinned_version: &'static str,
    /// 当前平台是否支持自动安装
    pub install_supported: bool,
    /// 自动安装目录
    pub install_path: String,
    /// 版本探测失败时的诊断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_error: Option<String>,
}

/// 【桌面操作】【状态查询】解析驱动路径并读取版本，不启动 MCP、不申请系统权限。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `configured_path`: 用户配置的驱动路径或文件名
///
/// 返回:
/// - 驱动探测结果
pub async fn status(paths: &SaiPaths, configured_path: &str) -> DriverStatus {
    let install_dir = install::install_dir(paths);
    let install_path = install_dir.display().to_string();
    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let install_supported = release::current_platform().is_some();
    // 1. 先解析路径，文件不存在时不启动驱动
    let resolved = detect::resolve_driver_path(configured_path, &install_dir);
    let installed = resolved.as_ref().is_some_and(|path| path.is_file());
    let mut version = None;
    let mut version_error = None;
    if installed {
        if let Some(path) = resolved.as_ref() {
            // 2. 只对已存在的二进制做 --version 探测
            match process::probe_version(path).await {
                Ok(value) => version = Some(value),
                Err(error) => version_error = Some(error.to_string()),
            }
        }
    }
    let version_ok = version
        .as_deref()
        .is_some_and(|value| version_at_least(value, MIN_VERSION));
    DriverStatus {
        platform,
        arch,
        configured_path: configured_path.trim().to_string(),
        resolved_path: resolved.map(|path| path.display().to_string()),
        installed,
        version,
        version_ok,
        min_version: MIN_VERSION,
        pinned_version: PINNED_VERSION,
        install_supported,
        install_path,
        version_error,
    }
}

/// 【桌面操作】【安装入口】供测试注入下载器。
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
    install::install_with_fetcher(paths, fetch)
}
