use super::{charge, error, result_value};
use crate::{
    host::PluginHost,
    runtime::{budget, control::CallControl},
    Capabilities, ExecutionLimits,
};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

/// 【插件目录】【创建绑定】目录创建复用二进制输出范围及可信写入许可
/// @param lua 虚拟机；api 为接口；host 为宿主；capabilities 为授权；limits 为预算；control 为状态
/// @returns 安装结果，初始化和只读调用不能创建目录
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let fs: Table = api.get("fs")?;
    fs.set(
        "create_dir",
        lua.create_async_function(move |lua, args: MultiValue| {
            let (host, capabilities, limits, control) = (
                host.clone(),
                capabilities.clone(),
                limits.clone(),
                control.clone(),
            );
            async move {
                let context = control.system_context()?;
                if args.len() != 1 {
                    return Err(mlua::Error::runtime(
                        "create_dir expects exactly one string path",
                    ));
                }
                let Some(Value::String(path)) = args.front() else {
                    return Err(mlua::Error::runtime("directory path must be a string"));
                };
                let path = path.to_str()?.to_string();
                capabilities
                    .binary
                    .check_write(&path, context.allow_writes)
                    .map_err(error)?;
                budget::checkpoint(&lua)?;
                charge(&control, limits.system_calls)?;
                let path = host
                    .create_directory(path, context, capabilities)
                    .await
                    .map_err(error)?;
                budget::checkpoint(&lua)?;
                result_value(&lua, &path, limits.output_bytes)
            }
        })?,
    )
}
