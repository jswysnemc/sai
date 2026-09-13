use super::buffer::Buffer;
use crate::document::{self, Options};
use mlua::{LuaSerdeExt, UserDataMethods, Value};

/// 【文档转换】【缓冲绑定】在回调内部处理二进制正文，仅把有界摘录送入 Lua
/// @param methods 二进制句柄方法注册器
/// @returns 无；正文、工作预留和执行预算均属于同一虚拟机
pub(super) fn install<M: UserDataMethods<Buffer>>(methods: &mut M) {
    methods.add_async_method("document", |lua, this, value: Value| async move {
        // 1. 【文档转换】【调用校验】先确认句柄代次及选项，不接受未知字段或字符串数量
        let data = this.data()?;
        let options: Options = if value == Value::Nil {
            Options::default()
        } else {
            lua.from_value(value)?
        };
        options.validate().map_err(super::super::system::error)?;
        let services = &this.services;
        let timeout_ms = options
            .timeout_ms
            .unwrap_or(services.limits.binary_timeout_ms)
            .min(services.limits.binary_timeout_ms);
        let output_bytes = services.limits.output_bytes;
        let working = document::working_bytes(data.bytes().len(), options.mode, output_bytes)
            .map_err(super::super::system::error)?;
        let reservation = services
            .budget
            .reserve(working)
            .map_err(super::super::system::error)?;
        services.charge()?;
        // 2. 【文档转换】【工作移交】排队、超时及取消期间，线程仍持有输入和完整工作额度
        let result =
            super::super::native::work(&lua, timeout_ms, "document render", move |budget| {
                let _reservation = reservation;
                document::render(data.bytes(), options, budget, output_bytes)
            })
            .await?;
        // 3. 【文档转换】【结果交付】复核原回调身份，只序列化已经通过字节限制的摘录
        services.check(this.generation)?;
        super::super::system::result_value(&lua, &result, output_bytes)
    });
}
