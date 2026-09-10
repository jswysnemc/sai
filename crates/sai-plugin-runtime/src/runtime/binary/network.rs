use super::{buffer::Buffer, BinaryServices};
use crate::host::HttpRequest;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;
use std::time::Duration;

/// 【插件二进制】【网络绑定】精确授权请求和匿名下载分开，正文共用独立字节预算。
/// @param lua 虚拟机；api 为 binary 表；services 为可信能力与预算
/// @returns 两个异步入口的安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    for (name, download) in [("request", false), ("download", true)] {
        let services = services.clone();
        api.set(
            name,
            lua.create_async_function(move |lua, value: Table| {
                let services = services.clone();
                async move {
                    let generation = services.generation()?;
                    let mut request: HttpRequest = lua.from_value(Value::Table(value))?;
                    super::super::http::validate_sizes_and_headers(
                        &request,
                        services.limits.output_bytes,
                    )?;
                    request.method = request.method.to_ascii_uppercase();
                    let context = services.control.system_context()?;
                    if download {
                        if request.method != "GET"
                            || request.body.is_some()
                            || !request.headers.is_empty()
                        {
                            return Err(mlua::Error::runtime(
                                "binary download requires an anonymous GET without headers or body",
                            ));
                        }
                        if !services.capabilities.binary.public_downloads {
                            services
                                .capabilities
                                .authorize_url(&request.url)
                                .map_err(super::super::system::error)?;
                        }
                    } else {
                        services
                            .capabilities
                            .authorize_request(&request.method, &request.url, context.allow_writes)
                            .map_err(super::super::system::error)?;
                    }
                    let available = services.budget.available();
                    if available == 0 {
                        return Err(mlua::Error::runtime(
                            "plugin retained binary buffers exceed size limit",
                        ));
                    }
                    request.max_bytes = request.max_bytes.clamp(1, available);
                    request.timeout_ms = request
                        .timeout_ms
                        .clamp(1, services.limits.binary_timeout_ms);
                    let max_bytes = request.max_bytes;
                    let timeout = Duration::from_millis(request.timeout_ms);
                    services.charge()?;
                    // 1. 【插件二进制】【取消边界】超时或调用取消时直接释放正在执行的网络 Future
                    let operation = async {
                        if download {
                            services
                                .host
                                .download_binary(request, services.capabilities.clone())
                                .await
                        } else {
                            services
                                .host
                                .http_binary(
                                    request,
                                    services.capabilities.clone(),
                                    context.allow_writes,
                                )
                                .await
                        }
                    };
                    let response = tokio::time::timeout(timeout, operation)
                        .await
                        .map_err(|_| mlua::Error::runtime("plugin binary request timed out"))?
                        .map_err(super::super::system::error)?;
                    services.check(generation)?;
                    if response.body.len() > max_bytes {
                        return Err(mlua::Error::runtime(
                            "plugin binary response exceeds size limit",
                        ));
                    }
                    let result = lua.create_table()?;
                    result.set("status", response.status)?;
                    result.set(
                        "headers",
                        super::super::system::result_value(
                            &lua,
                            &response.headers,
                            services.limits.output_bytes,
                        )?,
                    )?;
                    let data = services
                        .budget
                        .retain(response.body)
                        .map_err(super::super::system::error)?;
                    result.set("body", Buffer::new(data, services, generation)?)?;
                    Ok(result)
                }
            })?,
        )?;
    }
    Ok(())
}
