use super::{
    buffer::{self, Buffer},
    BinaryServices,
};
use mlua::{Lua, Table, Value};
use std::sync::Arc;

/// 【插件二进制】【构造入口】把有界 Lua 字符串转成共享预算内的字节缓冲
/// @param lua 虚拟机；api 为 binary 表；services 为可信调用和预算
/// @returns 原始字节与 Base64 构造入口的安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    let decoder = services.clone();
    api.set(
        "decode_base64",
        lua.create_function(move |_, text: mlua::LuaString| {
            let generation = decoder.generation()?;
            if text.as_bytes().len() > decoder.limits.output_bytes {
                return Err(mlua::Error::runtime(
                    "base64 text exceeds plugin input limit",
                ));
            }
            buffer::decode(&text.as_bytes(), decoder.clone(), generation)
        })?,
    )?;
    api.set(
        "from_bytes",
        lua.create_function(move |_, value: Value| {
            let generation = services.generation()?;
            let Value::String(raw) = value else {
                return Err(mlua::Error::runtime("binary raw input must be a string"));
            };
            let bytes = raw.as_bytes();
            if bytes.len() > services.limits.output_bytes {
                return Err(mlua::Error::runtime("raw bytes exceed plugin input limit"));
            }
            if bytes.len() > services.budget.available() {
                return Err(mlua::Error::runtime(
                    "plugin retained binary buffers exceed size limit",
                ));
            }
            // 1. 【插件二进制】【有界复制】完成输入和总量检查后才复制，零字节和无效 UTF-8 保持原样
            services.charge()?;
            let data = services
                .budget
                .retain(bytes.to_vec())
                .map_err(super::super::system::error)?;
            Buffer::new(data, services.clone(), generation)
        })?,
    )
}
