use super::BinaryServices;
use crate::sqlite::MAX_REQUEST_BYTES;
use mlua::{Lua, LuaSerdeExt, Value};
use serde::de::DeserializeOwned;

/// 【数据库快照】【输入解码】先拒绝非有限数字和过大对象，再校验完整 JSON 大小和字段类型
/// @param lua 虚拟机；value 为结构化请求；services 为可信输出预算
/// @returns 经过类型校验的请求，NaN 和无穷大不能隐式转换成 null
pub(super) fn decode<T: DeserializeOwned>(
    lua: &Lua,
    value: Value,
    services: &BinaryServices,
) -> mlua::Result<T> {
    let limit = services.limits.output_bytes.min(MAX_REQUEST_BYTES);
    let mut remaining = limit;
    // 1. 【数据库快照】【复制前校验】遍历原值时计量，避免先复制超限树或把非有限数字转换为空值
    inspect(lua, &value, 0, &mut remaining)?;
    let value: serde_json::Value = lua.from_value(value)?;
    let encoded = serde_json::to_vec(&value).map_err(super::super::system::error)?;
    // 2. 【数据库快照】【编码后校验】实际 JSON 还包含转义和结构字符，再验证完整字节数
    if encoded.len() > limit {
        return Err(mlua::Error::runtime("SQLite request exceeds input limit"));
    }
    super::super::budget::charge_bytes(lua, encoded.len())?;
    serde_json::from_value(value).map_err(super::super::system::error)
}

/// 【数据库快照】【原值检查】直接遍历 Lua 值，限制复制前的节点和文字总量
/// @param lua 虚拟机；value 为原始值；depth 为嵌套深度；remaining 为输入大小的剩余额度
/// @returns 合法时成功；循环表、过深结构和非有限数字立即失败
fn inspect(lua: &Lua, value: &Value, depth: usize, remaining: &mut usize) -> mlua::Result<()> {
    if depth > 8 {
        return Err(mlua::Error::runtime("SQLite request nesting exceeds limit"));
    }
    let size = match value {
        Value::String(text) => text.as_bytes().len().saturating_add(1),
        _ => 1,
    };
    *remaining = remaining
        .checked_sub(size)
        .ok_or_else(|| mlua::Error::runtime("SQLite request exceeds input limit"))?;
    super::super::budget::charge_bytes(lua, size)?;
    match value {
        Value::Number(number) if !number.is_finite() => {
            Err(mlua::Error::runtime("SQLite parameters must be finite"))
        }
        Value::Table(table) => {
            for entry in table.pairs::<Value, Value>() {
                let (key, value) = entry?;
                inspect(lua, &key, depth + 1, remaining)?;
                inspect(lua, &value, depth + 1, remaining)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
