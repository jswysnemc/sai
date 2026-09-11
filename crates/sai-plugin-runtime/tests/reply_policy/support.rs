#![allow(dead_code)]

use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::{HttpRequest, HttpResponse, PluginHost},
    Capabilities, ExecutionLimits, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::sync::Arc;

pub struct Host;

#[async_trait]
impl PluginHost for Host {
    /// 【回复策略测试】【网络隔离】纯策略测试不能访问外网
    /// @param request 请求；capabilities 为能力；allow_writes 为可信写入权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }
}

/// 【回复策略测试】【可信上下文】固定会话和操作标识，禁止通过计划跨会话投递
/// @param writable 当前宿主是否允许投递
/// @returns 独立调用上下文
pub fn context(writable: bool) -> InvocationContext {
    InvocationContext {
        session_id: "reply-session".into(),
        storage_session_id: "reply-session".into(),
        operation_id: "reply-operation".into(),
        workdir: "/trusted".into(),
        allow_writes: writable,
        ..Default::default()
    }
}

/// 【回复策略测试】【权限加载】使用独立声明与授权加载任意策略脚本
/// @param source 源码；declared 为清单许可；granted 为用户授权；change 为预算设置
/// @returns 已注册实例或加载错误
pub fn load(
    source: &str,
    declared: bool,
    granted: bool,
    change: impl FnOnce(&mut ExecutionLimits),
) -> Result<PluginRuntime> {
    let mut manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"reply-fixture","version":"1.0.0",
            "name":"Reply fixture","description":"Reply policy contract","entry":"init.lua",
            "capabilities":{"reply_policy":declared}
        })
        .to_string(),
    )?;
    change(&mut manifest.limits);
    let package = PluginPackage::new(manifest, [("init.lua".into(), source.into())].into())?;
    let granted = serde_json::from_value(json!({"reply_policy":granted}))?;
    PluginRuntime::load(package, json!({}), granted, Arc::new(Host))
}

pub const SOURCE: &str = r#"
local completed = 0
sai.register_reply_policy({
    --- 【回复策略测试】【准备】记录只读事实并创建本次投递资料
    --- @param input string 用户输入
    --- @param ctx table 可信调用上下文
    --- @return table 上下文、提醒及可选投递资料
    prepare=function(input,ctx)
        assert(ctx.allow_writes == false, "prepare must be read-only")
        if not ctx.reply_can_deliver then return {context="previous"} end
        return {context="previous",reminder=input,delivery={text=input}}
    end,
    --- 【回复策略测试】【完成】只有可信写入调用可以消费准备结果
    --- @param delivery table 本次投递资料
    --- @param ctx table 可信调用上下文
    --- @return table 投递完成后的上下文
    complete=function(delivery,ctx)
        assert(ctx.allow_writes == true, "delivery must be authorized")
        completed=completed+1
        return {context=delivery.text .. ":" .. completed}
    end,
})
"#;
