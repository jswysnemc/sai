use super::control::CallControl;
use super::registration::lua_error;
use crate::host::{HttpRequest, PluginHost};
use crate::{Capabilities, ExecutionLimits};
use chrono::{DateTime, FixedOffset, Utc};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::Ordering;
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
    install_time(lua, api)?;
    let http = lua.create_table()?;
    http.set(
        "request",
        lua.create_async_function(move |lua, value: Table| {
            let host = host.clone();
            let capabilities = capabilities.clone();
            let control = control.clone();
            let limits = limits.clone();
            async move {
                if !control.active.load(Ordering::Acquire) {
                    return Err(mlua::Error::runtime(
                        "HTTP is only available during plugin callbacks",
                    ));
                }
                let mut request: HttpRequest = lua.from_value(Value::Table(value))?;
                if request.url.len() > 8192
                    || request
                        .headers
                        .iter()
                        .map(|(name, value)| name.len().saturating_add(value.len()))
                        .sum::<usize>()
                        > 16 * 1024
                {
                    return Err(mlua::Error::runtime(
                        "plugin HTTP URL or headers exceed size limits",
                    ));
                }
                if request.headers.keys().any(|name| {
                    matches!(
                        name.to_ascii_lowercase().as_str(),
                        "host"
                            | "connection"
                            | "content-length"
                            | "transfer-encoding"
                            | "proxy-authorization"
                            | "proxy-connection"
                    )
                }) {
                    return Err(mlua::Error::runtime(
                        "plugin HTTP transport headers are host controlled",
                    ));
                }
                capabilities
                    .authorize_url(&request.url)
                    .map_err(lua_error)?;
                request.method = request.method.to_ascii_uppercase();
                if !matches!(request.method.as_str(), "GET" | "HEAD")
                    && !control.writable.load(Ordering::Acquire)
                {
                    return Err(mlua::Error::runtime(
                        "read-only plugin callback cannot make a writing HTTP request",
                    ));
                }
                if !matches!(
                    request.method.as_str(),
                    "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE"
                ) {
                    return Err(mlua::Error::runtime("unsupported plugin HTTP method"));
                }
                if request.headers.len() > 32
                    || request
                        .body
                        .as_ref()
                        .is_some_and(|body| body.len() > limits.output_bytes)
                {
                    return Err(mlua::Error::runtime(
                        "plugin HTTP request exceeds size limits",
                    ));
                }
                request.max_bytes = request.max_bytes.clamp(1, limits.output_bytes);
                request.timeout_ms = request.timeout_ms.clamp(1, 30_000);
                let timeout = std::time::Duration::from_millis(request.timeout_ms);
                let response = tokio::time::timeout(
                    timeout,
                    host.http(request, capabilities.http.iter().cloned().collect()),
                )
                .await
                .map_err(|_| mlua::Error::runtime("plugin HTTP request timed out"))?
                .map_err(lua_error)?;
                if response.text.len() > limits.output_bytes {
                    return Err(mlua::Error::runtime(
                        "plugin HTTP response exceeds size limit",
                    ));
                }
                lua.to_value(&response)
            }
        })?,
    )?;
    api.set("http", http)
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
