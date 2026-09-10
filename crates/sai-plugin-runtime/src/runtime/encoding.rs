use super::{budget, registration::lua_error};
use base64::Engine;
use mlua::{Lua, Table, Value};
use std::borrow::Cow;

/// 【插件】【字节编码】提供有界解码与 UTF-8 转换，二进制结果始终保留为 Lua 字符串
/// @param lua 虚拟机；api 为 sai 表；limit 为输入及输出字节上限
/// @returns 编码接口安装结果
pub(super) fn install(lua: &Lua, api: &Table, limit: usize) -> mlua::Result<()> {
    let encoding = lua.create_table()?;
    encoding.set(
        "decode",
        lua.create_function(move |lua, (format, input): (Value, Value)| {
            let (Value::String(format), Value::String(input)) = (format, input) else {
                return Err(mlua::Error::runtime(
                    "decode requires string format and input",
                ));
            };
            let bytes = input.as_bytes();
            check_size(bytes.len(), limit, "input")?;
            if format.as_bytes().len() > 16 {
                return Err(mlua::Error::runtime("unsupported encoding format"));
            }
            budget::charge_bytes(lua, bytes.len())?;
            let output = decode(&format.to_str()?, &bytes)?;
            check_size(output.len(), limit, "output")?;
            budget::checkpoint(lua)?;
            lua.create_string(output.as_ref())
        })?,
    )?;
    encoding.set(
        "to_utf8",
        lua.create_function(move |lua, (input, lossy): (Value, Value)| {
            let Value::String(input) = input else {
                return Err(mlua::Error::runtime("to_utf8 requires bytes as a string"));
            };
            let lossy = match lossy {
                Value::Nil => false,
                Value::Boolean(value) => value,
                _ => return Err(mlua::Error::runtime("to_utf8 lossy must be a boolean")),
            };
            let bytes = input.as_bytes();
            check_size(bytes.len(), limit, "input")?;
            budget::charge_bytes(lua, bytes.len())?;
            let text = if lossy {
                String::from_utf8_lossy(&bytes)
            } else {
                Cow::Borrowed(std::str::from_utf8(&bytes).map_err(lua_error)?)
            };
            check_size(text.len(), limit, "output")?;
            budget::checkpoint(lua)?;
            lua.create_string(text.as_bytes())
        })?,
    )?;
    api.set("encoding", encoding)
}

/// 【插件】【编码解析】按指定格式还原字节，去空白及文本容错策略由调用方决定
/// @param format 为 base64、hex 或 url；input 为原始编码文本字节
/// @returns 解码字节；非法 Base64、Hex 或未知格式返回错误
fn decode<'a>(format: &str, input: &'a [u8]) -> mlua::Result<Cow<'a, [u8]>> {
    match format {
        "base64" => base64::engine::general_purpose::STANDARD
            .decode(input)
            .map(Cow::Owned)
            .map_err(lua_error),
        "hex" => hex::decode(input).map(Cow::Owned).map_err(lua_error),
        "url" => Ok(urlencoding::decode_binary(input)),
        _ => Err(mlua::Error::runtime("unsupported encoding format")),
    }
}

/// 【插件】【编码限制】同时限制输入及 UTF-8 替换后可能增大的结果
/// @param bytes 为字节数；limit 为上限；stage 区分输入和输出
/// @returns 不超限时成功
fn check_size(bytes: usize, limit: usize, stage: &str) -> mlua::Result<()> {
    if bytes > limit {
        return Err(mlua::Error::runtime(format!(
            "encoding {stage} exceeds plugin size limit"
        )));
    }
    Ok(())
}
