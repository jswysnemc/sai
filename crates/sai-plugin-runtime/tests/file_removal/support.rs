#![allow(dead_code)]

use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, ExecutionLimits, InvocationContext, PluginManifest, PluginPackage,
    PluginRuntime,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

pub struct Call {
    pub request: FileRemovalRequest,
    pub context: SystemContext,
    pub capabilities: Capabilities,
}

#[derive(Default)]
pub struct Host {
    pub calls: Mutex<Vec<Call>>,
    pub result: AtomicBool,
    pub fail: AtomicBool,
    pub blocking: AtomicBool,
    pub active: AtomicUsize,
    pub entered: tokio::sync::Notify,
    pub released: tokio::sync::Notify,
}

struct Active<'a>(&'a Host);

impl Drop for Active<'_> {
    /// 【删除宿主测试】【取消观测】宿主 Future 释放时记录在途操作已经结束
    /// @returns 无；通知等待取消结果的测试
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

#[async_trait]
impl PluginHost for Host {
    /// 【删除宿主测试】【网络隔离】本地删除不允许触发网络访问
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【删除宿主测试】【请求记录】记录删除类型与真实调用边界，支持受控等待
    /// @param request 删除请求；context 为可信上下文；capabilities 为授权交集
    /// @returns 测试指定的布尔结果或错误
    async fn remove_file(
        &self,
        request: FileRemovalRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<bool> {
        self.calls.lock().unwrap().push(Call {
            request,
            context,
            capabilities,
        });
        self.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(self);
        self.entered.notify_one();
        if self.blocking.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixture removal failure");
        }
        Ok(self.result.load(Ordering::SeqCst))
    }
}

/// 【删除宿主测试】【完整声明】仅声明两类独立文件操作权限
/// @returns 不包含文件正文读取、写入及网络权限的能力集合
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"remove_paths":["output"],"trash_paths":["output"]}}))
        .unwrap()
}

/// 【删除宿主测试】【分项加载】允许分别设置清单、授权和执行限制
/// @param source 脚本；host 为宿主；declared 为声明；granted 为授权；change 为限制调整
/// @returns 可执行实例或初始化错误
pub fn load(
    source: &str,
    host: Arc<dyn PluginHost>,
    declared: Capabilities,
    granted: Capabilities,
    change: impl FnOnce(&mut ExecutionLimits),
) -> Result<PluginRuntime> {
    let mut manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"file-removal","version":"1.0.0","name":"File removal",
            "description":"File removal contract","entry":"init.lua","capabilities":declared
        })
        .to_string(),
    )?;
    change(&mut manifest.limits);
    let package = PluginPackage::new(manifest, [("init.lua".into(), source.into())].into())?;
    PluginRuntime::load(package, json!({}), granted, host)
}

/// 【删除宿主测试】【默认加载】以一致的声明和授权加载脚本
/// @param source 入口；host 为宿主；change 为限制调整
/// @returns 可执行实例
pub fn runtime(
    source: &str,
    host: Arc<dyn PluginHost>,
    change: impl FnOnce(&mut ExecutionLimits),
) -> PluginRuntime {
    load(source, host, capabilities(), capabilities(), change).unwrap()
}

/// 【删除宿主测试】【可信目录】构建独立于 Lua 可见字段的工作目录与权限
/// @param writable 是否允许写入
/// @returns 调用上下文
pub fn context(writable: bool) -> InvocationContext {
    InvocationContext {
        workdir: "/trusted".into(),
        allow_writes: writable,
        ..Default::default()
    }
}

pub const SOURCE: &str = r#"
    --- 【删除宿主测试】【请求转交】选择独立接口并尝试伪造可见权限
    --- @param args table 路径和操作
    --- @param ctx table 可见上下文
    --- @return boolean 宿主结果
    local function run(args,ctx)
        ctx.workdir="/forged"; ctx.allow_writes=true
        return sai.fs[args.operation or "remove_file"](args.path or "output/a")
    end
    sai.register_tool({name="run",description="Removal contract",access="optional_writes",parameters={type="object"},execute=run})
"#;
