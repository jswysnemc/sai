use super::BinaryServices;
use mlua::{Lua, Table};
use std::sync::Arc;
use std::time::Duration;

/// 【插件图片】【终端绑定】只开放尺寸查询与单张图片绘制，布局和业务反馈由 Lua 决定。
/// @param lua 虚拟机；api 为 sai 表；services 为可信授权和限制
/// @returns 终端接口安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    let terminal = lua.create_table()?;
    let size_services = services.clone();
    terminal.set(
        "size",
        lua.create_function(move |lua, ()| {
            authorize(&size_services)?;
            match size_services.host.terminal_size() {
                Some((columns, rows)) => {
                    let result = lua.create_table()?;
                    result.set("columns", columns)?;
                    result.set("rows", rows)?;
                    Ok(Some(result))
                }
                None => Ok(None),
            }
        })?,
    )?;
    terminal.set(
        "display_image",
        lua.create_async_function(move |lua, (path, size): (String, Option<String>)| {
            let services = services.clone();
            async move {
                authorize(&services)?;
                let generation = services.generation()?;
                if path.is_empty()
                    || path.len() > 4096
                    || size.as_ref().is_some_and(|value| value.len() > 32)
                {
                    return Err(mlua::Error::runtime("invalid image display path or size"));
                }
                let context = services.control.system_context()?;
                services.charge()?;
                let result = tokio::time::timeout(
                    Duration::from_millis(services.limits.binary_timeout_ms),
                    services
                        .host
                        .display_image(path, size, context, services.capabilities.clone()),
                )
                .await
                .map_err(|_| mlua::Error::runtime("plugin image display timed out"))?
                .map_err(super::super::system::error)?;
                services.check(generation)?;
                super::super::system::result_value(&lua, &result, services.limits.output_bytes)
            }
        })?,
    )?;
    api.set("terminal", terminal)
}

/// 【插件图片】【展示授权】尺寸和绘制均需处于有效调用且取得展示能力。
/// @param services 当前宿主服务
/// @returns 允许访问终端图片能力时成功
fn authorize(services: &BinaryServices) -> mlua::Result<()> {
    services.generation()?;
    if !services.capabilities.binary.display_images {
        return Err(mlua::Error::runtime("plugin image display is not allowed"));
    }
    Ok(())
}
