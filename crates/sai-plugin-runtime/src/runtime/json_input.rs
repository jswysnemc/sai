use super::budget;
use mlua::{Lua, Table, Value as LuaValue};
use serde_json::Value;
use std::sync::Arc;

const MAX_POINTER_BYTES: usize = 4096;

/// 【插件】【原始整数】为工具或事件绑定原输入的整数查询，保留 JSON 数字表示类型
/// @param lua 当前虚拟机；context 为回调上下文；input 为仅由当前执行持有的 JSON 快照
/// @returns 安装结果；查询返回精确十进制文本，缺失节点或非整数返回 nil
pub(super) fn install(lua: &Lua, context: &Table, input: &Arc<Value>) -> mlua::Result<()> {
    // 1. 【插件】【快照生命周期】弱引用避免 Lua 保存上下文时保留堆外的大型输入
    let input = Arc::downgrade(input);
    context.set(
        "json_integer",
        lua.create_function(move |lua, pointer: LuaValue| {
            let input = input
                .upgrade()
                .ok_or_else(|| mlua::Error::runtime("plugin JSON input context has expired"))?;
            let LuaValue::String(pointer) = pointer else {
                return Err(mlua::Error::runtime("JSON Pointer must be a UTF-8 string"));
            };
            if pointer.as_bytes().len() > MAX_POINTER_BYTES {
                return Err(mlua::Error::runtime("JSON Pointer exceeds 4096 bytes"));
            }
            let pointer = pointer.to_str()?;
            // 2. 【插件】【查询预算】路径扫描与结果创建共用本次计算额度
            budget::charge_bytes(lua, pointer.len())?;
            validate_pointer(&pointer)?;
            let result = match input.pointer(&pointer) {
                Some(Value::Number(number)) if !number.is_f64() => Some(number.to_string()),
                _ => None,
            };
            if let Some(text) = &result {
                budget::charge_bytes(lua, text.len())?;
            }
            budget::checkpoint(lua)?;
            Ok(result)
        })?,
    )
}

/// 【插件】【JSON 路径】校验 RFC 6901 路径前缀和转义，拒绝模糊或截断的路径
/// @param pointer 已通过长度和 UTF-8 检查的路径
/// @returns 空路径或合法 JSON Pointer 成功，其他输入返回错误
fn validate_pointer(pointer: &str) -> mlua::Result<()> {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(mlua::Error::runtime(
            "JSON Pointer must be empty or start with /",
        ));
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return Err(mlua::Error::runtime("invalid JSON Pointer escape"));
        }
    }
    Ok(())
}
