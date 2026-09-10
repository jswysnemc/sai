use super::{buffer::Buffer, BinaryServices};
use crate::host::VisionRequest;
use crate::runtime::services::{charge, check_size, service_error};
use mlua::{Lua, LuaSerdeExt, Table, UserDataMethods, Value};
use std::sync::Arc;
use std::time::Duration;

/// 【插件视觉】【信息入口】查询当前宿主提供的视觉模型，不能选择供应商或读取凭据。
/// @param lua 虚拟机；api 为公共接口；services 为授权及可信调用状态
/// @returns 模型查询入口安装结果
pub(super) fn install(lua: &Lua, api: &Table, services: Arc<BinaryServices>) -> mlua::Result<()> {
    let vision = lua.create_table()?;
    vision.set(
        "info",
        lua.create_function(move |lua, ()| {
            allowed(&services)?;
            let info = services
                .control
                .services()?
                .vision_info()
                .map_err(service_error)?;
            check_size(&info, services.limits.output_bytes, "vision model info")?;
            lua.to_value(&info)
        })?,
    )?;
    api.set("vision", vision)
}

/// 【插件视觉】【缓冲分析】只发送当前回调内有效句柄指向的图片，输入输出和请求次数均有上限。
/// @param methods 二进制缓冲方法集合
/// @returns 无；完成或取消后不得复用旧缓冲或模型服务
pub(super) fn install_methods<M: UserDataMethods<Buffer>>(methods: &mut M) {
    methods.add_async_method("analyze_image", |lua, this, options: Table| async move {
        allowed(&this.services)?;
        let data = this.data()?;
        let services = &this.services;
        let request: VisionRequest = lua.from_value(Value::Table(options))?;
        request
            .validate(data.bytes().len(), services.limits.output_bytes)
            .map_err(service_error)?;
        let model = services.control.services()?;
        charge(
            &services.control.model_requests,
            services.limits.model_requests,
            "model request",
        )?;
        let timeout = request
            .timeout_ms
            .unwrap_or(services.limits.timeout_ms)
            .clamp(1, services.limits.timeout_ms);
        // 1. 【插件视觉】【取消边界】模型 Future 随回调超时或取消释放，图片租约覆盖请求的整个生命周期
        let response = tokio::time::timeout(
            Duration::from_millis(timeout),
            model.analyze_image(request, data, services.limits.output_bytes),
        )
        .await
        .map_err(|_| mlua::Error::runtime("plugin vision request timed out"))?
        .map_err(service_error)?;
        services.check(this.generation)?;
        check_size(&response, services.limits.output_bytes, "vision response")?;
        lua.to_value(&response)
    });
}

/// 【插件视觉】【独立授权】视觉模型能力与普通模型、网络和终端能力分别校验。
/// @param services 本实例的有效能力及活动回调
/// @returns 已授权且回调有效时成功
fn allowed(services: &BinaryServices) -> mlua::Result<()> {
    if !services.capabilities.vision {
        return Err(mlua::Error::runtime(
            "plugin vision capability is not allowed",
        ));
    }
    services.generation()?;
    Ok(())
}
