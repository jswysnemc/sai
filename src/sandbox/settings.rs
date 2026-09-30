use crate::config::{SandboxConfig, SandboxNetworkMode};
use crate::paths::SaiPaths;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

/// 进程级沙箱配置快照：权限层判定与状态展示读取它，免去逐层传递配置。
#[derive(Debug, Clone, Default)]
pub(crate) struct SandboxSettings {
    /// 用户配置
    pub config: SandboxConfig,
    /// sai 自身持有凭据的文件，沙箱内始终隐藏
    pub protected_files: Vec<PathBuf>,
}

impl SandboxSettings {
    /// 按配置与 sai 路径构造快照。
    ///
    /// 参数:
    /// - `config`: 沙箱配置
    /// - `paths`: sai 路径
    ///
    /// 返回:
    /// - 配置快照
    pub(crate) fn new(config: &SandboxConfig, paths: &SaiPaths) -> Self {
        Self {
            config: config.clone(),
            protected_files: vec![
                paths.config_file.clone(),
                paths.secrets_file.clone(),
                paths.mcp_config_file(),
            ],
        }
    }
}

/// 返回全局快照存储。
fn store() -> &'static RwLock<SandboxSettings> {
    static STORE: OnceLock<RwLock<SandboxSettings>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(SandboxSettings::default()))
}

/// 【沙箱】【配置快照】加载或保存配置后更新进程级快照。
///
/// 参数:
/// - `config`: 沙箱配置
/// - `paths`: sai 路径
///
/// 返回:
/// - 无
pub(crate) fn install(config: &SandboxConfig, paths: &SaiPaths) {
    let settings = SandboxSettings::new(config, paths);
    match store().write() {
        Ok(mut guard) => *guard = settings,
        Err(poisoned) => *poisoned.into_inner() = settings,
    }
}

/// 返回当前沙箱配置快照。
///
/// 返回:
/// - 快照副本；从未安装时为默认配置
pub(crate) fn current() -> SandboxSettings {
    match store().read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// 判断当前平台是否有可用的沙箱实现。
///
/// 返回:
/// - Linux 与 macOS 返回 `true`
pub(crate) fn platform_supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos"))
}

/// 判断审核模式的 Shell 命令是否应进入沙箱。
///
/// 返回:
/// - 配置启用且平台支持时返回 `true`
pub(crate) fn sandbox_requested() -> bool {
    platform_supported() && current().config.enabled
}

/// 判断沙箱内是否保留网络。
///
/// 返回:
/// - 配置为 `allow` 时返回 `true`
pub(crate) fn network_allowed() -> bool {
    current().config.network == SandboxNetworkMode::Allow
}
