use crate::manifest::API_VERSION;
use mlua::{Lua, LuaSerdeExt, Table};

/// 当前宿主已经实现、可供插件探测的稳定功能名。
/// 只描述本构建有没有接口，不表示当前插件已被授权使用。
const HOST_FEATURES: &[&str] = &[
    "binary",
    "crypto",
    "document",
    "encoding",
    "fs",
    "http",
    "json",
    "model",
    "notifications",
    "notify",
    "permission_audit",
    "plugin_lock",
    "plugin_storage",
    "process",
    "reply_policy",
    "scheduler",
    "session_storage",
    "sqlite",
    "terminal_image",
    "text",
    "time",
    "tools",
    "tui_status",
    "vision",
    "workspace",
];

/// 【插件】【功能查询】安装不需要额外授权的宿主功能探测。
///
/// @param lua 虚拟机；api 为 sai 表
/// @returns 宿主信息函数的安装结果
pub(super) fn install(lua: &Lua, api: &Table) -> mlua::Result<()> {
    let host = lua.create_table()?;
    host.set("info", lua.create_function(|lua, ()| host_info(lua))?)?;
    host.set(
        "has",
        lua.create_function(|_, name: String| {
            if name.len() > 64 {
                return Err(mlua::Error::runtime("host feature name exceeds 64 bytes"));
            }
            Ok(HOST_FEATURES.binary_search(&name.as_str()).is_ok())
        })?,
    )?;
    api.set("host", host)
}

/// 组装当前构建的 API 版本和已实现功能。
fn host_info(lua: &Lua) -> mlua::Result<Table> {
    let info = lua.create_table()?;
    info.set("api_version", API_VERSION)?;
    let features = lua.create_table()?;
    for (index, name) in HOST_FEATURES.iter().enumerate() {
        features.set(index + 1, *name)?;
    }
    features.set_metatable(Some(lua.array_metatable()))?;
    info.set("features", features)?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::HOST_FEATURES;

    #[test]
    fn host_features_stay_sorted_and_unique() {
        let mut sorted = HOST_FEATURES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(HOST_FEATURES, sorted.as_slice());
    }
}
