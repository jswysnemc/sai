use crate::host::{PluginHost, StorageRequest};
use crate::runtime::{
    control::CallControl,
    system::{charge, error, result_value},
};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// 【插件】【存储绑定】私有记录独立授权，只读工具也可更新自身会话记录。
/// @param lua 虚拟机；api 为公开表；host 为宿主；capabilities 为授权；limits 为预算；control 为调用状态
/// @returns 三项原子操作的绑定结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
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
                move |lua, (key, first, second): (String, Option<Value>, Option<Value>)| {
                    let session = control.private_session(action != "get")?;
                    if !capabilities.system.session_storage {
                        return Err(mlua::Error::runtime(
                            "plugin session storage is not allowed",
                        ));
                    }
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
                    charge(&control, limits.system_calls)?;
                    let result = host
                        .storage(request, &session, &capabilities)
                        .map_err(error)?;
                    result_value(lua, &result, limits.output_bytes.min(256 * 1024))
                },
            )?,
        )?;
    }
    api.set("storage", storage)
}
