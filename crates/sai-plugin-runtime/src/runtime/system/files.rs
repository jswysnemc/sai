use super::{charge, error, result_value};
use crate::host::{FileReadRequest, PluginHost};
use crate::runtime::control::CallControl;
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// 【插件】【读取选项】文件读取不能覆盖目录或权限。
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReadOptions {
    max_bytes: Option<usize>,
    lossy: bool,
}

/// 【插件】【目录选项】限制单次目录枚举数量。
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct DirectoryOptions {
    max_entries: Option<usize>,
}

/// 【插件】【文件绑定】按当前目录与有效读取授权访问文件，所有结果均受大小限制。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 文件接口安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let fs = lua.create_table()?;
    let (read_host, read_capabilities, read_limits, read_control) = (
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    );
    fs.set(
        "read_text",
        lua.create_async_function(move |lua, (path, options): (String, Option<Table>)| {
            let (host, capabilities, limits, control) = (
                read_host.clone(),
                read_capabilities.clone(),
                read_limits.clone(),
                read_control.clone(),
            );
            async move {
                let context = control.system_context()?;
                capabilities
                    .system
                    .check_read_request(&path)
                    .map_err(error)?;
                let options: ReadOptions = options
                    .map(|table| lua.from_value(Value::Table(table)))
                    .transpose()?
                    .unwrap_or_default();
                let request = FileReadRequest {
                    path,
                    max_bytes: options
                        .max_bytes
                        .unwrap_or(1024 * 1024)
                        .clamp(1, limits.output_bytes),
                    lossy: options.lossy,
                };
                let max_bytes = request.max_bytes;
                charge(&control, limits.system_calls)?;
                let output = host
                    .read_text(request, context, capabilities)
                    .await
                    .map_err(error)?;
                if output.text.len() > max_bytes {
                    return Err(mlua::Error::runtime(
                        "plugin file text exceeds requested byte limit",
                    ));
                }
                result_value(&lua, &output, limits.output_bytes)
            }
        })?,
    )?;
    let (list_host, list_capabilities, list_limits, list_control) = (
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    );
    fs.set(
        "read_dir",
        lua.create_async_function(move |lua, (path, options): (String, Option<Table>)| {
            let (host, capabilities, limits, control) = (
                list_host.clone(),
                list_capabilities.clone(),
                list_limits.clone(),
                list_control.clone(),
            );
            async move {
                let context = control.system_context()?;
                capabilities
                    .system
                    .check_read_request(&path)
                    .map_err(error)?;
                let options: DirectoryOptions = options
                    .map(|table| lua.from_value(Value::Table(table)))
                    .transpose()?
                    .unwrap_or_default();
                let max_entries = options.max_entries.unwrap_or(256).clamp(1, 1024);
                charge(&control, limits.system_calls)?;
                let output = host
                    .read_directory(path, max_entries, context, capabilities)
                    .await
                    .map_err(error)?;
                if output.entries.len() > max_entries {
                    return Err(mlua::Error::runtime(
                        "plugin directory exceeds requested entry limit",
                    ));
                }
                result_value(&lua, &output, limits.output_bytes)
            }
        })?,
    )?;
    fs.set(
        "stat",
        lua.create_async_function(move |lua, path: String| {
            let (host, capabilities, limits, control) = (
                host.clone(),
                capabilities.clone(),
                limits.clone(),
                control.clone(),
            );
            async move {
                let context = control.system_context()?;
                capabilities
                    .system
                    .check_read_request(&path)
                    .map_err(error)?;
                charge(&control, limits.system_calls)?;
                let output = host
                    .file_info(path, context, capabilities)
                    .await
                    .map_err(error)?;
                match output {
                    Some(output) => result_value(&lua, &output, limits.output_bytes),
                    None => Ok(Value::Nil),
                }
            }
        })?,
    )?;
    api.set("fs", fs)
}
