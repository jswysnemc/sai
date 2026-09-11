use super::buffer::Buffer;
use crate::host::{BinaryConditionalWrite, BinaryRevision};
use mlua::{UserDataMethods, Value};
use std::time::Duration;

/// 【插件二进制】【条件写入绑定】同时要求有效读取范围、写入范围和可信回调写入权限
/// @param methods 二进制句柄的方法注册器
/// @returns 无；条件不匹配返回 false，权限及执行错误可以通过 pcall 捕获
pub(super) fn install<M: UserDataMethods<Buffer>>(methods: &mut M) {
    methods.add_async_method(
        "write_if",
        |_, this, (path, expected): (Value, Value)| async move {
            let data = this.data()?;
            let services = &this.services;
            let context = services.control.system_context()?;
            let Value::String(path) = path else {
                return Err(mlua::Error::runtime(
                    "plugin conditional write path must be a string",
                ));
            };
            let path = path.to_str()?;
            services
                .capabilities
                .binary
                .check_write(&path, context.allow_writes)
                .map_err(super::super::system::error)?;
            services
                .capabilities
                .system
                .check_read_request(&path)
                .map_err(super::super::system::error)?;
            let request = BinaryConditionalWrite {
                path: path.to_string(),
                expected: revision(expected)?,
                max_bytes: services.limits.binary_bytes,
            };
            // 1. 【插件二进制】【可信请求】调用参数只能选择文件和预期摘要，不能覆盖目录、权限或比较上限
            services.charge()?;
            let operation = services.host.write_binary_if(
                request,
                data,
                context,
                services.capabilities.clone(),
            );
            let matched = tokio::time::timeout(
                Duration::from_millis(services.limits.binary_timeout_ms),
                operation,
            )
            .await
            .map_err(|_| mlua::Error::runtime("plugin conditional binary write timed out"))?
            .map_err(super::super::system::error)?;
            services.check(this.generation)?;
            Ok(matched)
        },
    );
}

/// 【插件二进制】【修订解析】nil 表示不存在，摘要必须精确包含 64 个十六进制字符
/// @param value Lua 原始值，不进行数字或其他类型的字符串转换
/// @returns 明确的文件条件；非法摘要在宿主调用前失败
fn revision(value: Value) -> mlua::Result<BinaryRevision> {
    if matches!(value, Value::Nil) {
        return Ok(BinaryRevision::Missing);
    }
    let Value::String(text) = value else {
        return Err(mlua::Error::runtime(
            "expected SHA-256 must be nil or 64 hexadecimal characters",
        ));
    };
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(text.as_bytes(), &mut digest).map_err(|_| {
        mlua::Error::runtime("expected SHA-256 must be nil or 64 hexadecimal characters")
    })?;
    Ok(BinaryRevision::Sha256(digest))
}
