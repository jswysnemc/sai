use serde::Serialize;
use std::sync::OnceLock;

/// 沙箱后端探测结果。
#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub(crate) struct BackendAvailability {
    /// 后端名称：`bwrap`、`seatbelt` 或 `none`
    pub backend: &'static str,
    /// 后端能否真正启动隔离进程
    pub available: bool,
    /// 不可用时的原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 【沙箱】【后端探测】实际启动一次空命令确认后端可用，结果按进程缓存。
///
/// 只检查二进制存在并不够：Ubuntu 24.04 等发行版默认禁止非特权用户命名空间，
/// bwrap 能找到却无法建立沙箱。
///
/// 返回:
/// - 探测结果
pub(crate) fn backend_availability() -> BackendAvailability {
    static CACHE: OnceLock<BackendAvailability> = OnceLock::new();
    CACHE.get_or_init(probe).clone()
}

/// 执行一次后端探测。
#[cfg(target_os = "linux")]
fn probe() -> BackendAvailability {
    let result = std::process::Command::new("bwrap")
        .args([
            "--die-with-parent",
            "--unshare-net",
            "--unshare-pid",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--",
            "true",
        ])
        .stdin(std::process::Stdio::null())
        .output();
    probe_result("bwrap", result, "install bubblewrap (bwrap)")
}

/// 执行一次后端探测。
#[cfg(target_os = "macos")]
fn probe() -> BackendAvailability {
    let result = std::process::Command::new(super::macos_seatbelt::SANDBOX_EXEC)
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .stdin(std::process::Stdio::null())
        .output();
    probe_result("seatbelt", result, "sandbox-exec is missing")
}

/// 执行一次后端探测。
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn probe() -> BackendAvailability {
    BackendAvailability {
        backend: "none",
        available: false,
        reason: Some("no sandbox backend on this platform".to_string()),
    }
}

/// 把探测命令结果转换为可用性。
///
/// 参数:
/// - `backend`: 后端名称
/// - `result`: 探测命令输出
/// - `missing_hint`: 二进制缺失时的提示
///
/// 返回:
/// - 探测结果
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn probe_result(
    backend: &'static str,
    result: std::io::Result<std::process::Output>,
    missing_hint: &str,
) -> BackendAvailability {
    let reason = match result {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("sandbox probe failed")
                .trim()
                .to_string(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Some(missing_hint.to_string())
        }
        Err(error) => Some(error.to_string()),
    };
    BackendAvailability {
        backend,
        available: reason.is_none(),
        reason,
    }
}
