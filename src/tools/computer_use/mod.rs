//! 原生桌面操作入口：参考应用定向、快照句柄和窗口像素契约。

mod accessibility;
mod driver;
mod family;
mod frame;
mod response;
mod schema;
mod session;
mod snapshot_data;

#[cfg(test)]
mod accessibility_tests;
#[cfg(test)]
mod family_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(all(test, unix))]
mod protocol_stdio_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod snapshot_data_tests;
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

/// 【桌面操作】【工具识别】统一识别原有入口和分动作入口，避免遗漏权限或渐进分组。
/// 参数: `name` 为工具名；返回: 是否属于内置桌面工具族。
pub(crate) fn is_tool(name: &str) -> bool {
    name == TOOL_NAME || family::TOOLS.iter().any(|tool| tool.0 == name)
}

/// 【桌面操作】【动作名称】提供分动作入口的规范动作，供目录与终端复用。
/// 参数: `name` 为分动作工具名；返回: 规范动作，统一入口或未知工具返回空。
pub(crate) fn canonical_action(name: &str) -> Option<&'static str> {
    family::TOOLS
        .iter()
        .find(|tool| tool.0 == name)
        .map(|tool| tool.1)
}

/// 【桌面操作】【子任务隔离】移除桌面工具族，阻止子代理继承真实窗口和输入权限。
/// 参数: `registry` 为子代理候选工具表；返回: 无。
pub(crate) fn exclude_from_subagent(registry: &mut ToolRegistry) {
    registry.remove(TOOL_NAME);
    for (name, _, _) in family::TOOLS {
        registry.remove(name);
    }
}

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
    // 1. 【桌面操作】【工具族注册】所有命名入口共享会话，只有配置状态查询免桌面授权
    for (name, _, description) in family::TOOLS {
        let config = config.clone();
        let session = session.clone();
        let name = *name;
        let tool = ToolSpec::new_with_output(name,
            format!("{description} Requires explicit computer_use.enabled and OS permissions. All input is window-targeted, background-only and must use a fresh observation. Never retry uncertain input; observe again."),
            family::parameters(name), move |args| {
                let config = config.clone();
                let session = session.clone();
                async move {
                    let args = family::normalize(name, args)?;
                    schema::validate(&args)?;
                    if args["action"] == "status" { return Ok(status(&config)); }
                    ensure_enabled(&config)?;
                    let mut session = session.try_lock().map_err(|_| anyhow::anyhow!("computer_use_busy: another request owns this session; wait and observe again"))?;
                    session.execute(args).await
                }
            });
        registry.register(if name == "computer_status" {
            tool
        } else {
            tool.writes()
        });
    }
    // 2. 【桌面操作】【兼容入口】保留统一动作接口，与命名入口共享快照消费边界
    registry.register(ToolSpec::new_with_output(
        TOOL_NAME,
        "Control native desktop applications through the local cua-driver (not the built-in browser). Requires computer_use.enabled=true in user configuration and OS permissions; never change those settings yourself. Start with list_apps, then list_windows with an explicit app, then get_app_state with app and window_id. The state returns an accessibility tree, opaque element_index handles, a snapshot_id/frame_id and a window screenshot. Optional region and scale crop/downscale the screenshot; input points still refer to the delivered image and Sai projects them back to the original window. Every input must copy app, window_id and snapshot_id from that observation. Prefer element handles; never use desktop coordinates. Use get_value/get_text/locate_text to inspect current controls, or invoke/show_menu/set_value to interact with a returned handle. Each input consumes the snapshot: observe again before another action. Inputs use background delivery only; unavailable capture/input fails without foreground fallback. status inspects configuration without starting a driver; reset closes only this session. Do not retry uncertain actions after a timeout, cancellation or disconnect.",
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
