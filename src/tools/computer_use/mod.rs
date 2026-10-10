//! 原生桌面操作入口：参考应用定向、快照句柄和窗口像素契约。

mod driver;
mod response;
mod schema;
mod session;

#[cfg(all(test, unix))]
mod protocol_stdio_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use super::{ToolOutput, ToolPermission, ToolRegistry, ToolSpec};
use crate::config::ComputerUseConfig;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

pub(super) const TOOL_NAME: &str = "computer_use";

/// 【桌面操作】【工具注册】注册延迟启动的原生工具，桌面访问沿用 Sai 的权限审核。
/// @param registry 为工具表；config 为用户显式桌面配置
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry, config: &ComputerUseConfig) {
    static COORDINATOR: OnceLock<Arc<session::DesktopCoordinator>> = OnceLock::new();
    let coordinator = COORDINATOR
        .get_or_init(|| Arc::new(session::DesktopCoordinator::default()))
        .clone();
    let session =
        session::ComputerSession::new(Arc::new(driver::CuaDriver::new(config)), coordinator);
    register_session(registry, config, session);
}

/// 【桌面操作】【会话注册】将权限入口与已组合的原生会话绑定，测试可替换驱动边界。
/// @param registry 为工具表；config 为用户配置；session 为独占原生会话
/// @returns 无
fn register_session(
    registry: &mut ToolRegistry,
    config: &ComputerUseConfig,
    session: session::ComputerSession,
) {
    let config = config.clone();
    let session = Arc::new(Mutex::new(session));
    registry.register(ToolSpec::new_with_output(
        TOOL_NAME,
        "Control native desktop applications through the local cua-driver (not the built-in browser). Requires computer_use.enabled=true in user configuration and OS permissions; never change those settings yourself. Start with list_apps, then list_windows with an explicit app, then get_app_state with app and window_id. The state returns an accessibility tree, opaque element_index handles, a snapshot_id and a window screenshot. Every input must copy app, window_id and snapshot_id from that observation. Prefer element handles; x/y and drag coordinates are pixels in that exact screenshot, never screen coordinates. Each input consumes the snapshot: observe again before another action. Inputs use background delivery only; unavailable capture/input fails without foreground fallback. status inspects configuration without starting a driver; reset closes only this session. Do not retry uncertain actions after a timeout, cancellation or disconnect.",
        schema::parameters(),
        move |args| {
            let session = session.clone();
            let config = config.clone();
            async move {
                schema::validate(&args)?;
                if args["action"] == "status" {
                    return Ok(status(&config));
                }
                ensure_enabled(&config)?;
                let mut session = session.try_lock().map_err(|_| anyhow::anyhow!(
                    "computer_use_busy: another request owns this session; wait and observe again"
                ))?;
                session.execute(args).await
            }
        },
    ).writes().with_call_permission(call_permission));
}

/// 【桌面操作】【授权边界】只有本地配置检查免写入审核，截图也不能绕过桌面授权。
/// @param args 为工具参数
/// @returns 当前调用所需的权限类别
fn call_permission(args: &Value) -> ToolPermission {
    if args.get("action").and_then(Value::as_str) == Some("status") {
        ToolPermission::ReadOnly
    } else {
        ToolPermission::Writes
    }
}

/// 【桌面操作】【功能开关】在触及驱动之前检查用户开关、平台和配置。
/// @param config 为用户配置
/// @returns 可使用原生功能时成功
fn ensure_enabled(config: &ComputerUseConfig) -> Result<()> {
    if !config.enabled {
        bail!("Computer Use is disabled. Ask the user to enable computer_use.enabled in their Sai configuration; do not enable it yourself.");
    }
    if !matches!(std::env::consts::OS, "linux" | "macos" | "windows") {
        bail!("Computer Use is unavailable on {}", std::env::consts::OS);
    }
    config.validate()
}

/// 【桌面操作】【状态查询】报告配置而不启动驱动、读取应用或申请系统权限。
/// @param config 为当前配置
/// @returns 无桌面副作用的状态文本
fn status(config: &ComputerUseConfig) -> ToolOutput {
    ToolOutput::text(json!({
        "enabled":config.enabled,
        "driver_path":config.driver_path,
        "timeout_ms":config.timeout_ms,
        "platform":std::env::consts::OS,
        "note":"Configuration only; no OS permission or driver availability probe was performed. Native access requires cua-driver and user-granted OS permissions."
    }).to_string())
}
