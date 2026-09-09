use super::{charge, error};
use crate::host::PluginHost;
use crate::runtime::control::CallControl;
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, Table};
use std::sync::Arc;

/// 【插件】【环境绑定】公开平台标识和当前进程号，环境变量必须逐项授权且只能在回调内读取。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 元数据与环境接口的安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let system = lua.create_table()?;
    system.set("platform", std::env::consts::OS)?;
    system.set("process_id", std::process::id())?;
    api.set("system", system)?;
    let environment = lua.create_table()?;
    environment.set(
        "get",
        lua.create_function(move |_, name: String| {
            control.system_context()?;
            capabilities
                .system
                .authorize_environment(&name)
                .map_err(error)?;
            charge(&control, limits.system_calls)?;
            let value = host.environment(&name, &capabilities).map_err(error)?;
            if value
                .as_ref()
                .is_some_and(|value| value.len() > limits.output_bytes)
            {
                return Err(mlua::Error::runtime(
                    "plugin environment value exceeds size limit",
                ));
            }
            Ok(value)
        })?,
    )?;
    api.set("env", environment)
}
