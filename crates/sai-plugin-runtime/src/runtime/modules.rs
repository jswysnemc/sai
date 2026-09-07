use crate::PluginPackage;
use mlua::chunk::ChunkMode;
use mlua::{Lua, Table, Value};

/// 【插件】【模块加载】安装只读取源码快照的 require，不暴露磁盘或动态库加载器。
/// @param lua 插件虚拟机；package 为已经验证的源码快照
/// @returns require 安装结果
pub(super) fn install(lua: &Lua, package: &PluginPackage) -> mlua::Result<()> {
    let sources = package.sources.clone();
    let id = package.manifest.id.clone();
    lua.set_named_registry_value("sai.loaded_modules", lua.create_table()?)?;
    lua.set_named_registry_value("sai.loading_modules", lua.create_table()?)?;
    lua.globals().set(
        "require",
        lua.create_function(move |lua, name: String| {
            if name.len() > 200
                || name.split('.').any(|part| {
                    part.is_empty()
                        || !part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                })
            {
                return Err(mlua::Error::runtime(
                    "require accepts package-local module names only",
                ));
            }
            let loaded: Table = lua.named_registry_value("sai.loaded_modules")?;
            let existing: Value = loaded.raw_get(name.as_str())?;
            if !existing.is_nil() {
                return Ok(existing);
            }
            let loading: Table = lua.named_registry_value("sai.loading_modules")?;
            if loading.raw_get::<bool>(name.as_str())? {
                return Err(mlua::Error::runtime(format!(
                    "cyclic plugin module: {name}"
                )));
            }
            let path = format!("{}.lua", name.replace('.', "/"));
            let source = sources
                .get(&path)
                .ok_or_else(|| mlua::Error::runtime(format!("plugin module not found: {name}")))?;
            loading.raw_set(name.as_str(), true)?;
            let result = lua
                .load(source)
                .set_mode(ChunkMode::Text)
                .set_name(format!("@{id}/{path}"))
                .eval::<Value>();
            loading.raw_set(name.as_str(), false)?;
            let result = result?;
            let result = if result.is_nil() {
                Value::Boolean(true)
            } else {
                result
            };
            loaded.raw_set(name.as_str(), result.clone())?;
            Ok(result)
        })?,
    )
}
