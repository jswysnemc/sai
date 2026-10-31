use crate::config::AppConfig;
use crate::gateways::workspace::gateway_workspace_path;
use crate::paths::SaiPaths;
use crate::tools::ToolRegistry;
use anyhow::Result;

pub(super) const TOOL_NAMES: [&str; 5] = [
    "session_probe",
    "agent_probe",
    "mesh_send",
    "session_create",
    "session_activate",
];

/// 【微信网关】【通信配置】参数 config 为当前请求的运行期副本，返回无。
/// 仅为微信网关开启跨会话通信并补入固定工具组，不写回全局配置或更改其它会话
pub(super) fn configure(config: &mut AppConfig) {
    config.mesh.cross_session = true;
    if let Some(runtime) = config.agent_runtime.as_mut() {
        if runtime.exclusive || !runtime.enabled_tools.is_empty() {
            for name in TOOL_NAMES {
                if !runtime.enabled_tools.iter().any(|item| item == name) {
                    runtime.enabled_tools.push(name.into());
                }
            }
        }
    }
}

/// 【微信网关】【通信注册】参数为 registry、paths 和真实 session_id，返回注册结果。
/// 所有网格消息均以当前微信会话发出，过滤工具与重绑权限后仍保留跨会话能力
pub(super) fn register(
    registry: &mut ToolRegistry,
    paths: &SaiPaths,
    session_id: &str,
) -> Result<()> {
    let (_, state_dir) = crate::state::state_dir_for_workspace_session(
        paths,
        &gateway_workspace_path(paths),
        session_id,
    )?;
    let owner_key = state_dir.display().to_string();
    // 1. 【微信网关】【身份绑定】权限配置和工具闭包必须使用同一会话归属与跨会话开关
    registry.set_session_ownership(owner_key.clone(), session_id.into(), true);
    crate::tools::mesh::register(registry, paths.clone(), owner_key, session_id.into(), true);
    Ok(())
}

#[cfg(test)]
#[path = "mesh_tools_tests.rs"]
mod tests;
