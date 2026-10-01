//! 浏览器用户目录：默认持久保存登录状态与站点验证结果，被占用或不可写时退回临时目录。
//!
//! 同时缓存浏览器真实的 User-Agent。无头模式自带的 `HeadlessChrome` 标识会被
//! Cloudflare 等站点直接拦截，启动参数需要换成同版本的普通 Chrome 标识。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 指定持久用户目录的环境变量；取值 `temp` 时每次启动都使用临时目录。
pub(super) const PROFILE_ENV: &str = "SAI_BROWSER_PROFILE";
/// 环境变量中表示临时目录的取值。
const TEMPORARY_VALUE: &str = "temp";
/// 浏览器在用户目录中写出的调试端口文件。
pub(super) const ACTIVE_PORT_FILE: &str = "DevToolsActivePort";
/// User-Agent 缓存文件名，放在用户目录之外，清除浏览数据时保留。
const USER_AGENT_CACHE: &str = "user-agent.json";

/// 用户目录的使用方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProfileMode {
    /// 持久目录：Cookie、登录状态与站点验证跨进程保留
    Persistent,
    /// 临时目录：随浏览器关闭删除，用于测试与目录被占用时的兜底
    Temporary,
}

/// 本次启动实际使用的用户目录。
pub(super) enum ProfileDir {
    Persistent(PathBuf),
    Temporary(tempfile::TempDir),
}

impl ProfileDir {
    /// 【内置浏览器】【用户目录】返回目录路径。
    /// @returns 用户目录
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Persistent(path) => path,
            Self::Temporary(dir) => dir.path(),
        }
    }

    /// 【内置浏览器】【用户目录】判断是否为持久目录。
    /// @returns 持久目录时为 true
    pub(crate) fn is_persistent(&self) -> bool {
        matches!(self, Self::Persistent(_))
    }
}

/// 缓存的 User-Agent，按浏览器可执行文件与修改时间失效。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CachedUserAgent {
    executable: String,
    modified: u64,
    user_agent: String,
}

/// 【内置浏览器】【目录模式】按环境变量决定默认目录模式。
/// @returns `SAI_BROWSER_PROFILE=temp` 时为临时目录，否则为持久目录
pub(crate) fn default_mode() -> ProfileMode {
    match std::env::var(PROFILE_ENV) {
        Ok(value) if value.trim().eq_ignore_ascii_case(TEMPORARY_VALUE) => ProfileMode::Temporary,
        _ => ProfileMode::Persistent,
    }
}

/// 【内置浏览器】【目录定位】返回持久用户目录：环境变量优先，否则放在 Sai 数据目录下。
/// @returns 持久目录；无法确定数据目录时为空
pub(super) fn persistent_root() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(PROFILE_ENV)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case(TEMPORARY_VALUE))
    {
        return Some(PathBuf::from(value));
    }
    crate::paths::SaiPaths::new()
        .ok()
        .map(|paths| paths.data_dir.join("browser").join("profile"))
}

/// 【内置浏览器】【目录准备】按模式准备用户目录；持久目录先清掉上次遗留的端口文件。
/// @param mode 为目录模式
/// @returns 准备好的用户目录
pub(super) fn prepare(mode: ProfileMode) -> Result<ProfileDir> {
    if mode == ProfileMode::Persistent {
        if let Some(root) = persistent_root() {
            if prepare_persistent(&root).is_ok() {
                return Ok(ProfileDir::Persistent(root));
            }
        }
    }
    temporary()
}

/// 【内置浏览器】【临时目录】创建随浏览器关闭删除的临时用户目录。
/// @returns 临时用户目录
pub(super) fn temporary() -> Result<ProfileDir> {
    let dir = tempfile::Builder::new()
        .prefix("sai-browser-")
        .tempdir()
        .context("create browser profile directory")?;
    Ok(ProfileDir::Temporary(dir))
}

/// 【内置浏览器】【持久目录准备】创建目录并删除上次进程遗留的端口文件。
///
/// 端口文件不删的话，启动等待会读到已失效的旧端口。
///
/// @param root 为持久目录
/// @returns 准备结果
fn prepare_persistent(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("create browser profile {}", root.display()))?;
    let stale = root.join(ACTIVE_PORT_FILE);
    if stale.exists() {
        std::fs::remove_file(&stale)
            .with_context(|| format!("remove stale {}", stale.display()))?;
    }
    clear_download_history(root);
    Ok(())
}

/// 【内置浏览器】【下载记录清理】清空用户目录里的浏览器下载记录。
///
/// 无头 Chrome 在用户目录残留下载记录时，下一次下载会让整个浏览器崩溃，
/// 崩溃又留下新的未完成记录，之后每次下载都会失败。下载文件本身由 Sai
/// 的下载目录保存，浏览器自己的下载历史没有用处，启动前直接清空。
///
/// @param root 为持久用户目录
/// @returns 无；数据库不存在或清理失败时保持原样
pub(super) fn clear_download_history(root: &Path) {
    let history = root.join("Default").join("History");
    if !history.is_file() {
        return;
    }
    let Ok(connection) = rusqlite::Connection::open(&history) else {
        return;
    };
    // 各版本 Chrome 的表不完全相同，逐张清理，缺表不影响其余
    for table in ["downloads_url_chains", "downloads_slices", "downloads"] {
        let _ = connection.execute(&format!("DELETE FROM {table}"), []);
    }
}

/// 【内置浏览器】【浏览数据清除】删除持久用户目录，下次启动时得到全新的浏览器状态。
/// @returns 是否删除了目录
pub(crate) fn remove_persistent() -> Result<bool> {
    let Some(root) = persistent_root() else {
        return Ok(false);
    };
    if !root.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&root)
        .with_context(|| format!("remove browser profile {}", root.display()))?;
    Ok(true)
}

/// 【内置浏览器】【标识修正】把无头模式的 User-Agent 改成同版本普通 Chrome 的写法。
/// @param user_agent 为浏览器上报的 User-Agent
/// @returns 修正后的 User-Agent；本来就不含无头标识时为空
pub(super) fn normal_user_agent(user_agent: &str) -> Option<String> {
    user_agent
        .contains("HeadlessChrome")
        .then(|| user_agent.replace("HeadlessChrome", "Chrome"))
}

/// 【内置浏览器】【标识缓存】读取与当前浏览器匹配的 User-Agent 缓存。
/// @param executable 为浏览器可执行文件
/// @returns 缓存的 User-Agent；不存在或浏览器已更新时为空
pub(super) fn cached_user_agent(executable: &Path) -> Option<String> {
    let file = cache_file()?;
    let cached: CachedUserAgent =
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    (cached.executable == executable.display().to_string()
        && cached.modified == modified_seconds(executable)?)
    .then_some(cached.user_agent)
}

/// 【内置浏览器】【标识缓存】写入当前浏览器的 User-Agent，供后续启动直接使用。
/// @param executable 为浏览器可执行文件；user_agent 为修正后的 User-Agent
/// @returns 无；写入失败只影响下次启动多一次探测
pub(super) fn store_user_agent(executable: &Path, user_agent: &str) {
    let (Some(file), Some(modified)) = (cache_file(), modified_seconds(executable)) else {
        return;
    };
    let cached = CachedUserAgent {
        executable: executable.display().to_string(),
        modified,
        user_agent: user_agent.to_string(),
    };
    if let (Some(parent), Ok(text)) = (file.parent(), serde_json::to_string(&cached)) {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(&file, text);
    }
}

/// 【内置浏览器】【标识缓存】缓存文件位置：持久用户目录的同级目录。
/// @returns 缓存文件路径
fn cache_file() -> Option<PathBuf> {
    Some(persistent_root()?.parent()?.join(USER_AGENT_CACHE))
}

/// 【内置浏览器】【文件时间】读取文件修改时间（秒）。
/// @param path 为文件路径
/// @returns 修改时间；读取失败时为空
fn modified_seconds(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    )
}
