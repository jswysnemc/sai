use super::{
    budget,
    control::CallControl,
    system::{charge, error, result_value},
};
use crate::host::{NotificationRequest, PluginHost};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;
use std::time::Duration;

/// 【插件通知】【Lua 绑定】只在可信写入回调中开放平台投递，并复用系统调用预算。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为预算；control 为状态
/// @returns 通知接口绑定结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let notify = lua.create_table()?;
    notify.set(
        "send",
        lua.create_async_function(move |lua, input: Value| {
            let (host, capabilities, limits, control) = (
                host.clone(),
                capabilities.clone(),
                limits.clone(),
                control.clone(),
            );
            async move {
                // 1. 【插件通知】【可信权限】上下文来源于 Rust，Lua 字段不能扩大写入权限
                let context = control.system_context()?;
                let mut request: NotificationRequest = lua.from_value(input)?;
                request
                    .authorize(&capabilities, context.allow_writes)
                    .map_err(error)?;
                request.timeout_ms = request.timeout_ms.min(limits.timeout_ms);
                budget::checkpoint(&lua)?;
                charge(&control, limits.system_calls)?;
                // 2. 【插件通知】【调用期限】单次等待不能扩大回调时限，超时会释放宿主 Future
                let desktop = request.desktop;
                let sound = request.sound.is_some();
                let result = tokio::time::timeout(
                    Duration::from_millis(request.timeout_ms),
                    host.notify(request, context, capabilities),
                )
                .await
                .map_err(|_| mlua::Error::runtime("plugin notification delivery timed out"))?
                .map_err(error)?;
                budget::checkpoint(&lua)?;
                if result.desktop != desktop || result.sound != sound {
                    return Err(mlua::Error::runtime(
                        "notification host returned inconsistent delivery channels",
                    ));
                }
                result_value(&lua, &result, limits.output_bytes)
            }
        })?,
    )?;
    api.set("notify", notify)
}
