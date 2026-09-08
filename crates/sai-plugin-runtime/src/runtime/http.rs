use super::control::CallControl;
use super::registration::lua_error;
use crate::host::{HttpRequest, PluginHost};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件】【HTTP 绑定】安装受来源、方法、字节限制和调用生命周期约束的请求入口。
/// @param lua 虚拟机；api 为 sai 表；host 为宿主；capabilities 为有效授权；limits 为资源上限；control 为调用状态
/// @returns HTTP 接口安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
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
                validate_sizes_and_headers(&request, limits.output_bytes)?;
                request.method = request.method.to_ascii_uppercase();
                let allow_writes = control.writable.load(Ordering::Acquire);
                capabilities
                    .authorize_request(&request.method, &request.url, allow_writes)
                    .map_err(lua_error)?;
                request.max_bytes = request.max_bytes.clamp(1, limits.output_bytes);
                request.timeout_ms = request.timeout_ms.clamp(1, limits.http_timeout_ms);
                let max_bytes = request.max_bytes;
                let timeout = std::time::Duration::from_millis(request.timeout_ms);
                // 1. 【插件】【请求生命周期】超时或外部取消直接回收宿主 Future，不遗留后台请求
                let response =
                    tokio::time::timeout(timeout, host.http(request, capabilities, allow_writes))
                        .await
                        .map_err(|_| mlua::Error::runtime("plugin HTTP request timed out"))?
                        .map_err(lua_error)?;
                if response.text.len() > max_bytes {
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

/// 【插件】【请求边界】在网络调用前验证 URL、头部及正文大小，保留传输头控制权。
/// @param request 待发送请求；output_limit 为包声明的字节上限
/// @returns 请求满足资源与传输约束时成功
fn validate_sizes_and_headers(request: &HttpRequest, output_limit: usize) -> mlua::Result<()> {
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
    if request.headers.len() > 32
        || request
            .body
            .as_ref()
            .is_some_and(|body| body.len() > output_limit)
    {
        return Err(mlua::Error::runtime(
            "plugin HTTP request exceeds size limits",
        ));
    }
    Ok(())
}
