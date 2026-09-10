use crate::host::{PluginHost, StorageRequest};
use crate::runtime::{
    budget,
    control::CallControl,
    system::{charge, error, result_value},
};
use crate::{Capabilities, ExecutionLimits};
use mlua::{FromLua, Lua, LuaSerdeExt, Table, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件】【存储作用域】会话记录保留原权限语义，跨会话记录另需显式写入权限。
#[derive(Clone, Copy)]
enum Scope {
    Session,
    Plugin,
}

/// 【插件】【存储绑定】分别安装会话记录和插件持久记录，权限与命名空间互不继承。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为预算；control 为调用状态
/// @returns 两种作用域的原子操作绑定结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let storage = table(
        lua,
        host.clone(),
        capabilities.clone(),
        limits.clone(),
        control.clone(),
        Scope::Session,
    )?;
    storage.set(
        "plugin",
        table(lua, host, capabilities, limits, control, Scope::Plugin)?,
    )?;
    api.set("storage", storage)
}

/// 【插件】【存储操作】复用键值校验与系统预算，始终从 Rust 状态取得作用域和权限。
/// @param lua 虚拟机；host 为宿主；capabilities 为授权；limits 为预算；control 为状态；scope 为固定作用域
/// @returns 包含读取、写入和比较交换函数的表
fn table(
    lua: &Lua,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
    scope: Scope,
) -> mlua::Result<Table> {
    let storage = lua.create_table()?;
    for action in ["get", "set", "compare_exchange"] {
        let (host, capabilities, limits, control) = (
            host.clone(),
            capabilities.clone(),
            limits.clone(),
            control.clone(),
        );
        storage.set(
            action,
            lua.create_function(
                move |lua, (key, first, second): (Value, Option<Value>, Option<Value>)| {
                    // 1. 【插件存储】【可信权限】初始化与事件写入先拒绝，再检查独立能力和调用权限
                    let mutation = action != "get";
                    let session = control.private_session(mutation)?;
                    let allow_writes = control.writable.load(Ordering::Acquire);
                    match scope {
                        Scope::Session if !capabilities.system.session_storage => {
                            return Err(mlua::Error::runtime(
                                "plugin session storage is not allowed",
                            ));
                        }
                        Scope::Plugin => {
                            if !capabilities.system.plugin_storage {
                                return Err(mlua::Error::runtime("plugin storage is not allowed"));
                            }
                            if mutation && !allow_writes {
                                return Err(mlua::Error::runtime(
                                    "read-only plugin callback cannot mutate plugin storage",
                                ));
                            }
                            budget::checkpoint(lua)?;
                        }
                        _ => {}
                    }
                    // 2. 【插件存储】【输入校验】新接口只接受字符串键，旧接口保留既有字符串转换行为
                    if matches!(scope, Scope::Plugin) && !matches!(key, Value::String(_)) {
                        return Err(mlua::Error::runtime("plugin storage key must be a string"));
                    }
                    let key = String::from_lua(key, lua)?;
                    crate::host::validate_storage_key(&key).map_err(error)?;
                    let first: serde_json::Value = lua.from_value(first.unwrap_or(Value::Nil))?;
                    let second: serde_json::Value = lua.from_value(second.unwrap_or(Value::Nil))?;
                    for value in [&first, &second] {
                        if serde_json::to_vec(value).map_err(error)?.len() > 256 * 1024 {
                            return Err(mlua::Error::runtime(
                                "plugin storage value exceeds 256 KiB",
                            ));
                        }
                    }
                    let request = match action {
                        "get" => StorageRequest::Get { key },
                        "set" => StorageRequest::Set { key, value: first },
                        _ => StorageRequest::CompareExchange {
                            key,
                            expected: first,
                            value: second,
                        },
                    };
                    // 3. 【插件存储】【调用期限】输入转换之后再次校验时限，避免失效回调继续提交写入
                    if matches!(scope, Scope::Plugin) {
                        budget::checkpoint(lua)?;
                    }
                    charge(&control, limits.system_calls)?;
                    let result = match scope {
                        Scope::Session => host.storage(request, &session, &capabilities),
                        Scope::Plugin => host.plugin_storage(request, &capabilities, allow_writes),
                    }
                    .map_err(error)?;
                    if matches!(scope, Scope::Plugin) {
                        budget::checkpoint(lua)?;
                    }
                    result_value(lua, &result, limits.output_bytes.min(256 * 1024))
                },
            )?,
        )?;
    }
    Ok(storage)
}
