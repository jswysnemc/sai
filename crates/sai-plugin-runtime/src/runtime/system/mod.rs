mod environment;
mod files;
mod process;

use super::control::CallControl;
use crate::host::PluginHost;
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件】【系统绑定】安装文件、环境和模板进程接口，共用本次回调的授权与预算。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 系统接口安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    environment::install(
        lua,
        api,
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    )?;
    files::install(
        lua,
        api,
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    )?;
    process::install(lua, api, host, capabilities, limits, control)
}

/// 【插件】【系统预算】只有通过输入和权限校验的调用才消耗一次额度。
/// @param control 当前调用；limit 为系统调用次数上限
/// @returns 仍有可用额度时成功
fn charge(control: &CallControl, limit: usize) -> mlua::Result<()> {
    control
        .system_calls
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < limit).then_some(count + 1)
        })
        .map(|_| ())
        .map_err(|_| mlua::Error::runtime("plugin system call budget exceeded"))
}

/// 【插件】【系统结果】在结果进入 Lua 堆之前检查序列化后的大小。
/// @param lua 虚拟机；value 为宿主结果；limit 为字节上限
/// @returns 可传入 Lua 的有界结果
fn result_value(lua: &Lua, value: &impl serde::Serialize, limit: usize) -> mlua::Result<Value> {
    let bytes = serde_json::to_vec(value).map_err(error)?;
    if bytes.len() > limit {
        return Err(mlua::Error::runtime(
            "plugin system output exceeds size limit",
        ));
    }
    lua.to_value(value)
}

/// 【插件】【系统错误】保留宿主错误原因链，便于 Lua 记录缺失证据。
/// @param error 原始宿主错误
/// @returns Lua 可捕获错误
fn error(error: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(format!("{error:#}"))
}
