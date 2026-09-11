use super::{charge, error};
use crate::host::{FileRemovalKind, FileRemovalRequest, PluginHost};
use crate::runtime::{budget, control::CallControl};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

/// 【插件文件】【删除绑定】为两类单文件操作安装严格参数与独立权限检查
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为限制；control 为调用状态
/// @returns 两个文件接口的安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let fs: Table = api.get("fs")?;
    for (name, kind) in [
        ("remove_file", FileRemovalKind::Permanent),
        ("trash_file", FileRemovalKind::Trash),
    ] {
        let (host, capabilities, limits, control) = (
            host.clone(),
            capabilities.clone(),
            limits.clone(),
            control.clone(),
        );
        fs.set(
            name,
            lua.create_async_function(move |lua, arguments: MultiValue| {
                let (host, capabilities, limits, control) = (
                    host.clone(),
                    capabilities.clone(),
                    limits.clone(),
                    control.clone(),
                );
                async move {
                    // 1. 【插件文件】【参数边界】路径严格接受一个 UTF-8 字符串，不接受选项或隐式转换
                    let context = control.system_context()?;
                    if arguments.len() != 1 {
                        return Err(mlua::Error::runtime(
                            "file removal expects exactly one string path",
                        ));
                    }
                    let Some(Value::String(path)) = arguments.front() else {
                        return Err(mlua::Error::runtime("file removal path must be a string"));
                    };
                    if path.as_bytes().len() > 4096 {
                        return Err(mlua::Error::runtime("file removal path exceeds size limit"));
                    }
                    let path = path.to_str()?.to_string();
                    capabilities
                        .system
                        .check_removal_request(&path, kind, context.allow_writes)
                        .map_err(error)?;
                    // 2. 【插件文件】【调用额度】通过语法与权限检查后扣除一次共享系统额度
                    budget::checkpoint(&lua)?;
                    charge(&control, limits.system_calls)?;
                    let removed = host
                        .remove_file(FileRemovalRequest { path, kind }, context, capabilities)
                        .await
                        .map_err(error)?;
                    budget::checkpoint(&lua)?;
                    Ok(removed)
                }
            })?,
        )?;
    }
    Ok(())
}
