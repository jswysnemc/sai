//! 【沙箱】Shell 命令的操作系统级隔离。
//!
//! Linux 使用 bubblewrap，macOS 使用 Seatbelt（sandbox-exec）；其余平台不隔离，
//! 由权限层的逐条审批兜底。策略按单次命令解析，配置快照随配置加载更新。

// Windows 没有沙箱后端，命令包装与策略字段只在状态展示与测试中出现
#![cfg_attr(windows, allow(dead_code))]

mod availability;
mod denial;
mod env_scrub;
#[cfg(any(target_os = "linux", test))]
mod linux_bwrap;
#[cfg(any(target_os = "macos", test))]
mod macos_seatbelt;
mod policy;
mod protected_paths;
mod settings;
mod status;
mod temp_dir;

#[cfg(all(test, target_os = "linux"))]
mod linux_bwrap_tests;

pub(crate) use availability::backend_availability;
pub(crate) use denial::{denial_report, detect_denial};
pub(crate) use policy::{FileAccess, SandboxPolicy};
#[cfg(test)]
pub(crate) use settings::SandboxSettings;
pub(crate) use settings::{
    current as current_settings, install, network_allowed, sandbox_requested,
};
pub(crate) use status::{sandbox_status, SandboxStatus};

use anyhow::{bail, Result};
use tokio::process::Command;

/// 【沙箱】【命令包装】把 Shell 命令包进当前平台的沙箱。
///
/// 参数:
/// - `policy`: 已解析策略
/// - `shell`: 沙箱内使用的 Shell
/// - `command`: Shell 命令文本
///
/// 返回:
/// - 已设置环境变量与工作目录的沙箱命令；后端不可用时返回错误
pub(crate) fn wrap_shell(policy: &SandboxPolicy, shell: &str, command: &str) -> Result<Command> {
    // 1. 后端不可用时直接说明原因，不再落到含糊的「找不到 shell」
    let backend = backend_availability();
    if !backend.available {
        bail!(
            "{} ({}: {}). {}",
            crate::i18n::text("sandbox backend is unavailable", "沙箱后端不可用"),
            backend.backend,
            backend.reason.as_deref().unwrap_or_default(),
            crate::i18n::text(
                "Set sandbox.enabled=false to run audited commands without isolation, or rerun with sandbox_permissions=\"require_escalated\".",
                "可设置 sandbox.enabled=false 取消隔离，或使用 sandbox_permissions=\"require_escalated\" 重新执行。",
            )
        );
    }
    // 2. 构造平台命令
    let mut process = platform_command(policy, shell, command)?;
    // 3. 只传入清理后的环境变量
    process.env_clear();
    process.envs(policy.env.iter().map(|(key, value)| (key, value)));
    process.current_dir(&policy.cwd);
    Ok(process)
}

/// 构造 Linux 沙箱命令。
#[cfg(target_os = "linux")]
fn platform_command(policy: &SandboxPolicy, shell: &str, command: &str) -> Result<Command> {
    let mut process = Command::new("bwrap");
    process.args(linux_bwrap::bwrap_args(policy, shell, command));
    Ok(process)
}

/// 构造 macOS 沙箱命令。
#[cfg(target_os = "macos")]
fn platform_command(policy: &SandboxPolicy, shell: &str, command: &str) -> Result<Command> {
    let mut process = Command::new(macos_seatbelt::SANDBOX_EXEC);
    process
        .arg("-p")
        .arg(macos_seatbelt::seatbelt_profile(policy))
        .arg(shell)
        .arg("-lc")
        .arg(command);
    Ok(process)
}

/// 其余平台没有沙箱实现。
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_command(_policy: &SandboxPolicy, _shell: &str, _command: &str) -> Result<Command> {
    bail!("sandboxed commands are not supported on this platform")
}
