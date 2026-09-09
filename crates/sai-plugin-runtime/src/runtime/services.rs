use super::control::CallControl;
use super::registration::lua_error;
use crate::host::{HostTool, ModelRequest, ModelRole};
use crate::{Capabilities, ExecutionLimits, ToolAccess};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 【插件】【调查能力】绑定单次模型请求与显式工具调用，业务循环仍由 Lua 控制。
/// @param lua 虚拟机；api 为 sai 表；capabilities 为有效授权；limits 为资源限制；control 为当前调用
/// @returns 全部调查接口的安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    install_model(
        lua,
        api,
        capabilities.clone(),
        limits.clone(),
        control.clone(),
    )?;
    install_tools(lua, api, capabilities, limits, control)
}

/// 【插件】【模型绑定】每次请求重新检查授权、输入与预算，并支持超时取消。
/// @param lua 虚拟机；api 为 sai 表；capabilities 为授权；limits 为限制；control 为调用状态
/// @returns 模型接口安装结果
fn install_model(
    lua: &Lua,
    api: &Table,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let model = lua.create_table()?;
    model.set(
        "complete",
        lua.create_async_function(move |lua, value: Table| {
            let capabilities = capabilities.clone();
            let limits = limits.clone();
            let control = control.clone();
            async move {
                if !capabilities.model {
                    return Err(mlua::Error::runtime(
                        "plugin model capability is not allowed",
                    ));
                }
                let services = control.services()?;
                let request: ModelRequest = lua.from_value(Value::Table(value))?;
                if request.messages.is_empty()
                    || request.messages.len() > 256
                    || request.tools.len() > 128
                {
                    return Err(mlua::Error::runtime(
                        "plugin model request exceeds message or tool count limits",
                    ));
                }
                if request
                    .messages
                    .last()
                    .is_none_or(|message| message.role != ModelRole::User)
                {
                    return Err(mlua::Error::runtime(
                        "plugin model request must end with a user message",
                    ));
                }
                check_size(&request, limits.output_bytes, "model request")?;
                if !request.tools.is_empty() {
                    let available = allowed_tools(
                        services.tools().map_err(service_error)?,
                        &capabilities,
                        &control,
                    );
                    for name in &request.tools {
                        if !available.iter().any(|tool| tool.name == *name) {
                            return Err(mlua::Error::runtime(format!(
                                "plugin model tool is not available: {name}"
                            )));
                        }
                    }
                }
                charge(
                    &control.model_requests,
                    limits.model_requests,
                    "model request",
                )?;
                let timeout = request
                    .timeout_ms
                    .unwrap_or(limits.timeout_ms)
                    .clamp(1, limits.timeout_ms);
                let response = tokio::time::timeout(
                    Duration::from_millis(timeout),
                    services.complete(request, limits.output_bytes),
                )
                .await
                .map_err(|_| mlua::Error::runtime("plugin model request timed out"))?
                .map_err(service_error)?;
                check_size(&response, limits.output_bytes, "model response")?;
                lua.to_value(&response)
            }
        })?,
    )?;
    api.set("model", model)
}

/// 【插件】【工具绑定】查询和调用都只使用当前回调可见的精确工具名称。
/// @param lua 虚拟机；api 为 sai 表；capabilities 为授权；limits 为限制；control 为调用状态
/// @returns 工具接口安装结果
fn install_tools(
    lua: &Lua,
    api: &Table,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let tools = lua.create_table()?;
    let list_capabilities = capabilities.clone();
    let list_control = control.clone();
    let output_limit = limits.output_bytes;
    tools.set(
        "list",
        lua.create_function(move |lua, ()| {
            if !list_control.active.load(Ordering::Acquire) {
                return Err(mlua::Error::runtime(
                    "tool catalog is only available during plugin callbacks",
                ));
            }
            if list_capabilities.tools.is_empty() {
                return lua.to_value(&Vec::<HostTool>::new());
            }
            let services = list_control.services()?;
            let available = allowed_tools(
                services.tools().map_err(service_error)?,
                &list_capabilities,
                &list_control,
            );
            check_size(&available, output_limit, "tool catalog")?;
            lua.to_value(&available)
        })?,
    )?;
    tools.set(
        "call",
        lua.create_async_function(move |lua, (name, arguments): (String, Value)| {
            let capabilities = capabilities.clone();
            let control = control.clone();
            let limits = limits.clone();
            async move {
                if !capabilities.tools.contains(&name) {
                    return Err(mlua::Error::runtime(format!(
                        "plugin tool capability is not allowed: {name}"
                    )));
                }
                let services = control.services()?;
                let available = allowed_tools(
                    services.tools().map_err(service_error)?,
                    &capabilities,
                    &control,
                );
                if !available.iter().any(|tool| tool.name == name) {
                    return Err(mlua::Error::runtime(format!(
                        "plugin tool is not available: {name}"
                    )));
                }
                let arguments: serde_json::Value = lua.from_value(arguments)?;
                let arguments = match arguments {
                    serde_json::Value::String(text) => text,
                    value @ serde_json::Value::Object(_) => {
                        serde_json::to_string(&value).map_err(lua_error)?
                    }
                    _ => {
                        return Err(mlua::Error::runtime(
                            "plugin tool arguments must be an object or JSON text",
                        ))
                    }
                };
                if arguments.len() > limits.output_bytes {
                    return Err(mlua::Error::runtime(
                        "plugin tool arguments exceed size limit",
                    ));
                }
                charge(&control.tool_calls, limits.tool_calls, "tool call")?;
                let output = services
                    .call_tool(&name, &arguments)
                    .await
                    .map_err(service_error)?;
                if output.len() > limits.output_bytes {
                    return Err(mlua::Error::runtime(
                        "plugin tool output exceeds size limit",
                    ));
                }
                Ok(output)
            }
        })?,
    )?;
    api.set("tools", tools)
}

/// 【插件】【工具交集】从宿主目录中筛选声明、授权及回调权限共同允许的工具。
/// @param tools 宿主目录；capabilities 为有效授权；control 为真实回调权限
/// @returns 保持宿主顺序的可见工具
fn allowed_tools(
    tools: Vec<HostTool>,
    capabilities: &Capabilities,
    control: &CallControl,
) -> Vec<HostTool> {
    let writable = control.writable.load(Ordering::Acquire);
    tools
        .into_iter()
        .filter(|tool| {
            capabilities.tools.contains(&tool.name)
                && (writable || tool.access == ToolAccess::ReadOnly)
        })
        .collect()
}

/// 【插件】【调用预算】在宿主操作开始前消耗一次预算，失败操作也计入次数。
/// @param counter 已调用次数；limit 为上限；kind 为错误中的操作名称
/// @returns 有剩余额度时成功
fn charge(counter: &AtomicUsize, limit: usize, kind: &str) -> mlua::Result<()> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            (count < limit).then_some(count + 1)
        })
        .map(|_| ())
        .map_err(|_| mlua::Error::runtime(format!("plugin {kind} budget exceeded")))
}

/// 【插件】【服务错误】保留嵌套插件的完整错误原因，避免只显示外层回调失败。
/// @param error 模型或工具服务的原始错误
/// @returns Lua 可捕获的错误及完整原因链
fn service_error(error: anyhow::Error) -> mlua::Error {
    mlua::Error::runtime(format!("{error:#}"))
}

/// 【插件】【数据大小】按实际 JSON 字节数检查宿主请求和结果。
/// @param value 待传递数据；limit 为字节上限；kind 为操作名称
/// @returns 数据未超过限制时成功
fn check_size(value: &impl serde::Serialize, limit: usize, kind: &str) -> mlua::Result<()> {
    if serde_json::to_vec(value).map_err(lua_error)?.len() > limit {
        return Err(mlua::Error::runtime(format!(
            "plugin {kind} exceeds size limit"
        )));
    }
    Ok(())
}
