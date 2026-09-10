use super::buffer::Buffer;
use mlua::{UserDataMethods, Value};
use sha2::{Digest, Sha256};

/// 【插件二进制】【数据检查】注册有界原始字节读取和 SHA-256 摘要，不读取额外文件。
/// @param methods 当前缓冲的 Lua 方法集合
/// @returns 无；摘要线程继续持有缓冲预算，失效句柄不能取回结果
pub(super) fn install<M: UserDataMethods<Buffer>>(methods: &mut M) {
    methods.add_method("bytes", |lua, this, (offset, length): (Value, Value)| {
        let data = this.data()?;
        let offset = index(offset)?;
        let length = index(length)?;
        if length > this.services.limits.output_bytes {
            return Err(mlua::Error::runtime(
                "binary byte range exceeds output limit",
            ));
        }
        let start = offset.min(data.bytes().len());
        let end = start.saturating_add(length).min(data.bytes().len());
        lua.create_string(&data.bytes()[start..end])
    });
    methods.add_async_method("sha256", |_, this, ()| async move {
        let data = this.data()?;
        this.services.charge()?;
        // 1. 【插件二进制】【摘要预算】大缓冲计算交给阻塞线程，取消不提前归还仍被使用的数据预算
        let digest =
            tokio::task::spawn_blocking(move || format!("{:x}", Sha256::digest(data.bytes())))
                .await
                .map_err(super::super::system::error)?;
        this.services.check(this.generation)?;
        Ok(digest)
    });
}

/// 【插件二进制】【范围参数】拒绝负数、分数和字符串，避免 Lua 数值转换静默截断读取范围。
/// @param value Lua 传入的偏移或长度
/// @returns 当前平台可表示的非负整数
fn index(value: Value) -> mlua::Result<usize> {
    match value {
        Value::Integer(value) => usize::try_from(value).ok(),
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= 0.0
                && value < 2_f64.powi(usize::BITS as i32) =>
        {
            Some(value as usize)
        }
        _ => None,
    }
    .ok_or_else(|| mlua::Error::runtime("binary byte range must use non-negative integers"))
}
