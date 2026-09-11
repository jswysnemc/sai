use super::{buffer::Buffer, BinaryServices};
use mlua::{Lua, Table, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct ReadOptions {
    max_bytes: Option<u64>,
    timeout_ms: Option<u64>,
}

/// 【插件二进制】【文件绑定】使用已有读取授权，将完整本地文件保存到受控缓冲
/// @param lua 虚拟机；api 为 binary 表；services 为可信上下文及共享预算
/// @returns 异步读取入口的安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    api.set(
        "read_file",
        lua.create_async_function(move |lua, (path, options): (Value, Value)| {
            let services = services.clone();
            async move {
                // 1. 【插件二进制】【读取边界】路径与选项不能覆盖工作目录和调用权限
                let generation = services.generation()?;
                let context = services.control.system_context()?;
                let Value::String(path) = path else {
                    return Err(mlua::Error::runtime(
                        "plugin binary read path must be a string",
                    ));
                };
                let path = path.to_str()?;
                services
                    .capabilities
                    .system
                    .check_read_request(&path)
                    .map_err(super::super::system::error)?;
                let path = path.to_string();
                let options = read_options(options)?;
                let available = services.budget.available();
                if available == 0 {
                    return Err(mlua::Error::runtime(
                        "plugin retained binary buffers exceed size limit",
                    ));
                }
                let max_bytes = options
                    .max_bytes
                    .unwrap_or(1024 * 1024)
                    .min(available as u64) as usize;
                let timeout_ms = options
                    .timeout_ms
                    .unwrap_or(services.limits.binary_timeout_ms)
                    .min(services.limits.binary_timeout_ms);
                super::super::budget::checkpoint(&lua)?;
                services.charge()?;
                // 2. 【插件二进制】【读取预留】把额度交给实际工作线程，取消 Future 不会提前释放线程持有的缓冲
                let buffer = services
                    .budget
                    .reserve(max_bytes)
                    .map_err(super::super::system::error)?;
                let operation =
                    services
                        .host
                        .read_binary(path, buffer, context, services.capabilities.clone());
                let data = tokio::time::timeout(Duration::from_millis(timeout_ms), operation)
                    .await
                    .map_err(|_| mlua::Error::runtime("plugin binary read timed out"))?
                    .map_err(super::super::system::error)?;
                // 3. 【插件二进制】【结果复核】拒绝超限结果和来自其他虚拟机的缓冲
                services.check(generation)?;
                super::super::budget::checkpoint(&lua)?;
                if data.bytes().len() > max_bytes {
                    return Err(mlua::Error::runtime(
                        "plugin binary file exceeds size limit",
                    ));
                }
                if !data.belongs_to(&services.budget) {
                    return Err(mlua::Error::runtime(
                        "plugin binary read result belongs to a different binary budget",
                    ));
                }
                Buffer::new(data, services, generation)
            }
        })?,
    )
}

/// 【插件二进制】【读取选项】只接受读取上限和时限，不解释元表或可伪造的上下文字段
/// @param value Lua 可选选项表
/// @returns 已验证的正整数选项，未知字段或非表值返回错误
fn read_options(value: Value) -> mlua::Result<ReadOptions> {
    let mut options = ReadOptions::default();
    let table = match value {
        Value::Nil => return Ok(options),
        Value::Table(table) => table,
        _ => {
            return Err(mlua::Error::runtime(
                "plugin binary read options must be a table",
            ))
        }
    };
    for entry in table.pairs::<Value, Value>() {
        let (key, value) = entry?;
        let Value::String(key) = key else {
            return Err(mlua::Error::runtime("unknown plugin binary read option"));
        };
        match key.as_bytes().as_ref() {
            b"max_bytes" => options.max_bytes = Some(positive_integer(value)?),
            b"timeout_ms" => options.timeout_ms = Some(positive_integer(value)?),
            _ => return Err(mlua::Error::runtime("unknown plugin binary read option")),
        }
    }
    Ok(options)
}

/// 【插件二进制】【整数选项】拒绝负数、零、分数、字符串以及不能表示为 u64 的数值
/// @param value 原始 Lua 数值
/// @returns 正整数，后续再按宿主预算收窄
fn positive_integer(value: Value) -> mlua::Result<u64> {
    match value {
        Value::Integer(value) if value > 0 => Ok(value as u64),
        Value::Number(value)
            if value.is_finite()
                && value > 0.0
                && value.fract() == 0.0
                && value < 2_f64.powi(64) =>
        {
            Ok(value as u64)
        }
        _ => Err(mlua::Error::runtime(
            "plugin binary read limits must be positive integers",
        )),
    }
}
