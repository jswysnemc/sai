use super::BinaryServices;
use crate::host::{binary::BinarySlot, BinaryData};
use base64::Engine;
use mlua::{UserData, UserDataMethods};
use std::sync::Arc;
use std::time::Duration;

/// 【插件二进制】【Lua 缓冲】数据不实现序列化，只允许有界查看、解码及授权写入。
pub(super) struct Buffer {
    data: Arc<BinarySlot>,
    services: Arc<BinaryServices>,
    generation: u64,
}

impl Buffer {
    /// 【插件二进制】【句柄创建】将预算租约和本回调代次绑定到 Lua 句柄。
    /// @param data 有预算的数据；services 为宿主接口；generation 为本次代次
    /// @returns 不可跨回调读取的句柄
    pub(super) fn new(
        data: BinaryData,
        services: Arc<BinaryServices>,
        generation: u64,
    ) -> mlua::Result<Self> {
        let data = Arc::new(std::sync::Mutex::new(Some(data)));
        if generation != 0 {
            services
                .control
                .binary_buffers
                .lock()
                .map_err(|_| mlua::Error::runtime("binary buffer registry lock poisoned"))?
                .push(Arc::downgrade(&data));
        }
        Ok(Self {
            data,
            services,
            generation,
        })
    }

    /// 【插件二进制】【句柄读取】先验证调用代次，再借用未释放的数据。
    /// @returns 本次有效数据；已关闭或过期句柄返回错误
    fn data(&self) -> mlua::Result<BinaryData> {
        self.services.check(self.generation)?;
        self.data
            .lock()
            .map_err(|_| mlua::Error::runtime("binary buffer lock poisoned"))?
            .clone()
            .ok_or_else(|| mlua::Error::runtime("binary buffer is closed"))
    }
}

impl UserData for Buffer {
    /// 【插件二进制】【缓冲方法】安装纯读取与带授权的异步写入，关闭操作只释放引用。
    /// @param methods 句柄方法注册器
    /// @returns 无；原始字节不会直接暴露给 Lua
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("len", |_, this, ()| Ok(this.data()?.bytes().len()));
        methods.add_method("close", |_, this, ()| {
            this.data
                .lock()
                .map_err(|_| mlua::Error::runtime("binary buffer lock poisoned"))?
                .take();
            Ok(())
        });
        methods.add_method("text", |_, this, requested: Option<usize>| {
            let data = this.data()?;
            let bytes = data.bytes();
            let limit = requested
                .unwrap_or(this.services.limits.output_bytes)
                .min(this.services.limits.output_bytes);
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(limit)]);
            let mut end = text.len().min(limit);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            Ok(text[..end].to_string())
        });
        methods.add_method("json_type", |_, this, pointer: String| {
            let data = this.data()?;
            let raw = super::json::select(data.bytes(), &pointer)?;
            Ok(raw.map(super::json::kind))
        });
        methods.add_method("json_string", |_, this, pointer: String| {
            let data = this.data()?;
            let raw = super::json::select(data.bytes(), &pointer)?;
            raw.filter(|raw| super::json::kind(raw) == "string")
                .map(|raw| {
                    super::json::string(raw, this.services.limits.output_bytes)
                        .map(|s| s.into_owned())
                })
                .transpose()
        });
        methods.add_method("json_base64", |_, this, pointer: String| {
            let data = this.data()?;
            let raw = super::json::select(data.bytes(), &pointer)?;
            let Some(raw) = raw.filter(|raw| super::json::kind(raw) == "string") else {
                return Ok(None);
            };
            if raw.get().as_bytes().contains(&b'\\')
                && raw.get().len().saturating_sub(2) > this.services.budget.available()
            {
                return Err(mlua::Error::runtime(
                    "escaped binary JSON string exceeds available buffer budget",
                ));
            }
            let text = super::json::string(raw, this.services.limits.binary_bytes)?;
            // 1. 【插件二进制】【转义预算】普通 Base64 借用响应，JSON 转义产生的副本也占用预算
            let scratch = match text {
                std::borrow::Cow::Borrowed(text) => {
                    return decode(text.as_bytes(), this.services.clone(), this.generation)
                        .map(Some)
                }
                std::borrow::Cow::Owned(text) => this
                    .services
                    .budget
                    .retain(text.into_bytes())
                    .map_err(super::super::system::error)?,
            };
            decode(scratch.bytes(), this.services.clone(), this.generation).map(Some)
        });
        methods.add_async_method("write", |lua, this, path: String| async move {
            let data = this.data()?;
            let services = &this.services;
            let context = services.control.system_context()?;
            services
                .capabilities
                .binary
                .check_write(&path, context.allow_writes)
                .map_err(super::super::system::error)?;
            services.charge()?;
            let operation =
                services
                    .host
                    .write_binary(path, data, context, services.capabilities.clone());
            let result = tokio::time::timeout(
                Duration::from_millis(services.limits.binary_timeout_ms),
                operation,
            )
            .await
            .map_err(|_| mlua::Error::runtime("plugin binary write timed out"))?
            .map_err(super::super::system::error)?;
            services.check(this.generation)?;
            super::super::system::result_value(&lua, &result, services.limits.output_bytes)
        });
    }
}

/// 【插件二进制】【Base64 解码】先限制预计分配，再登记解码后的实际字节。
/// @param text 编码字节；services 为预算；generation 为本次调用
/// @returns 拥有独立租约的新缓冲
pub(super) fn decode(
    text: &[u8],
    services: Arc<BinaryServices>,
    generation: u64,
) -> mlua::Result<Buffer> {
    services.check(generation)?;
    let padding = text
        .iter()
        .rev()
        .take(2)
        .take_while(|byte| **byte == b'=')
        .count();
    if base64::decoded_len_estimate(text.len()).saturating_sub(padding)
        > services.budget.available()
    {
        return Err(mlua::Error::runtime(
            "decoded binary buffer exceeds size limit",
        ));
    }
    services.charge()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| mlua::Error::runtime("invalid base64 binary data"))?;
    let data = services
        .budget
        .retain(bytes)
        .map_err(super::super::system::error)?;
    Buffer::new(data, services, generation)
}
