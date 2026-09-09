use super::control::CallControl;
use super::registration::lua_error;
use crate::host::PluginHost;
use crate::{Capabilities, ExecutionLimits};
use chrono::{DateTime, FixedOffset, Utc};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// 【插件】【宿主绑定】安装 JSON、文本、时间和受授权的 HTTP 接口。
/// @param lua 为虚拟机；api 为 sai 表；host 为宿主实现；capabilities 为有效授权；limits 为上限；control 为执行状态
/// @returns 全部宿主接口的安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    install_json(lua, api, limits.output_bytes)?;
    super::text::install(lua, api, limits.output_bytes)?;
    install_token_estimation(lua, api, host.clone(), limits.output_bytes)?;
    install_time(lua, api)?;
    super::services::install(
        lua,
        api,
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    )?;
    super::http::install(lua, api, host, capabilities, limits, control)
}

/// 【插件】【文本分词】使用宿主分词器，保持业务统计和主会话使用同一估算口径。
/// @param lua 虚拟机；api 为 sai 表；host 为纯文本能力实现；limit 为输入字节限制
/// @returns 文本分词函数的安装结果
fn install_token_estimation(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    limit: usize,
) -> mlua::Result<()> {
    let text: Table = api.get("text")?;
    text.set(
        "estimate_tokens",
        lua.create_function(move |_, value: String| {
            if value.len() > limit {
                return Err(mlua::Error::runtime(
                    "token estimation input exceeds plugin size limit",
                ));
            }
            host.estimate_tokens(&value).map_err(lua_error)
        })?,
    )
}

/// 【插件】【JSON 绑定】保留空数组和 null 的类型，避免 Lua 空表产生歧义。
/// @param lua 虚拟机；api 为 sai 表；limit 为 JSON 字节限制
/// @returns JSON 帮助函数安装结果
fn install_json(lua: &Lua, api: &Table, limit: usize) -> mlua::Result<()> {
    let json = lua.create_table()?;
    json.set("null", lua.null())?;
    json.set(
        "decode",
        lua.create_function(move |lua, text: String| {
            if text.len() > limit {
                return Err(mlua::Error::runtime("JSON input exceeds plugin size limit"));
            }
            let value: serde_json::Value = serde_json::from_str(&text).map_err(lua_error)?;
            lua.to_value(&value)
        })?,
    )?;
    json.set(
        "encode",
        lua.create_function(move |lua, value: Value| {
            let value: serde_json::Value = lua.from_value(value)?;
            let text = serde_json::to_string(&value).map_err(lua_error)?;
            if text.len() > limit {
                return Err(mlua::Error::runtime(
                    "JSON output exceeds plugin size limit",
                ));
            }
            Ok(text)
        })?,
    )?;
    json.set(
        "array",
        lua.create_function(|lua, table: Option<Table>| {
            let table = match table {
                Some(table) => table,
                None => lua.create_table()?,
            };
            table.set_metatable(Some(lua.array_metatable()))?;
            Ok(table)
        })?,
    )?;
    api.set("json", json)
}

/// 【插件】【时间绑定】提供时间戳和明确时区的 ISO 时间格式。
/// @param lua 虚拟机；api 为 sai 表
/// @returns 时间帮助函数安装结果
fn install_time(lua: &Lua, api: &Table) -> mlua::Result<()> {
    let time = lua.create_table()?;
    time.set(
        "now",
        lua.create_function(|_, ()| Ok(Utc::now().timestamp()))?,
    )?;
    time.set(
        "iso",
        lua.create_function(|_, (seconds, offset): (i64, Option<i32>)| {
            let offset = FixedOffset::east_opt(offset.unwrap_or(0))
                .ok_or_else(|| mlua::Error::runtime("invalid timezone offset"))?;
            Ok(DateTime::from_timestamp(seconds, 0)
                .map(|time| time.with_timezone(&offset).to_rfc3339()))
        })?,
    )?;
    api.set("time", time)
}
