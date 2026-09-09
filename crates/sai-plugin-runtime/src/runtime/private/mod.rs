mod storage;
mod workspace;

use super::control::CallControl;
use crate::{host::PluginHost, Capabilities, ExecutionLimits};
use mlua::{Lua, Table};
use std::sync::Arc;

/// 【插件】【私有绑定】安装会话存储和受管工作目录接口。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为预算；control 为调用状态
/// @returns 绑定结果，初始化阶段仍不能执行任何私有 I/O
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    storage::install(
        lua,
        api,
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    )?;
    workspace::install(lua, api, host, capabilities, limits, control)
}
