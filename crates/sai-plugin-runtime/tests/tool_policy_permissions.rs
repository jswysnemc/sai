#[path = "reply_policy/support.rs"]
mod support;
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, PluginManifest, PluginPackage, PluginRuntime, ToolPolicyInput,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Default)]
struct Host {
    mutations: AtomicUsize,
}

#[async_trait]
impl PluginHost for Host {
    /// 【工具策略权限测试】【网络隔离】测试不访问外网
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【工具策略权限测试】【状态观察】只读请求返回固定值，任何进入宿主的写入都会记录
    /// @param request 存储操作；session 为会话；capabilities 为授权
    /// @returns 固定值
    fn storage(&self, request: StorageRequest, _: &str, _: &Capabilities) -> Result<Value> {
        if !matches!(request, StorageRequest::Get { .. }) {
            self.mutations.fetch_add(1, Ordering::SeqCst);
        }
        Ok(json!({"saved":true}))
    }

    /// 【工具策略权限测试】【持久写入观察】用于证明拒绝发生在进入宿主之前
    /// @param request 存储操作；capabilities 为授权；allow_writes 为权限
    /// @returns 固定值
    fn plugin_storage(&self, _: StorageRequest, _: &Capabilities, _: bool) -> Result<Value> {
        self.mutations.fetch_add(1, Ordering::SeqCst);
        Ok(json!(true))
    }
}

#[derive(Default)]
struct Services {
    calls: AtomicUsize,
}

#[async_trait]
impl InvocationServices for Services {
    /// 【工具策略权限测试】【工具目录观察】外层传入的服务不应进入观察回调
    /// @returns 空目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![])
    }

    /// 【工具策略权限测试】【工具观察】检测越权的宿主工具调用
    /// @param name 工具名；arguments 为参数
    /// @returns 固定正文
    async fn call_tool(&self, _: &str, _: &str) -> Result<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("unexpected".into())
    }

    /// 【工具策略权限测试】【模型观察】检测越权的模型请求
    /// @param request 请求；max_bytes 为预算
    /// @returns 固定响应
    async fn complete(&self, _: ModelRequest, _: usize) -> Result<ModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModelResponse {
            content: "unexpected".into(),
            ..Default::default()
        })
    }
}

/// 【工具策略权限测试】【可信只读】伪造 Lua 上下文无法写入状态，也无法取得外层工具和模型服务
/// @returns 无；正常只读查询仍成功，所有越权操作在宿主之前拒绝
#[tokio::test]
async fn tool_policy_cannot_mutate_private_state_or_reuse_host_services() {
    let manifest = PluginManifest::parse(&json!({
        "api_version":1,"id":"tool-policy-readonly","version":"1.0.0","name":"Readonly policy",
        "description":"Trusted observation boundary","entry":"init.lua",
        "capabilities":{"reply_policy":true,"model":true,"tools":["probe"],"system":{"session_storage":true,"plugin_storage":true}}
    }).to_string()).unwrap();
    let source = r#"
    sai.register_reply_policy({prepare=function()end,complete=function()end,after_tool=function(input,state,ctx)
        ctx.allow_writes=true
        local probes={
            function()sai.storage.set('key',{forged=true})end,
            function()sai.storage.compare_exchange('key',nil,{forged=true})end,
            function()sai.storage.plugin.set('key',{forged=true})end,
            function()sai.storage.plugin.compare_exchange('key',nil,{forged=true})end,
            function()sai.tools.call('probe',{})end,
            function()sai.model.complete({messages={{role='user',content='probe'}}})end,
        }
        for _,probe in ipairs(probes)do local ok=pcall(probe); assert(not ok,'forbidden operation succeeded')end
        return {state=sai.storage.get('key')}
    end})
    "#;
    let host = Arc::new(Host::default());
    let services = Arc::new(Services::default());
    let grants = manifest.capabilities.clone();
    let plugin = PluginRuntime::load(
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap(),
        json!({}),
        grants,
        host.clone(),
    )
    .unwrap();
    let mut ctx = support::context(true);
    ctx.services = Some(services.clone());
    let result = plugin
        .after_tool(
            ToolPolicyInput {
                name: "probe".into(),
                local_name: None,
                arguments: json!({}),
                ok: true,
                tools: vec![],
            },
            Value::Null,
            ctx,
        )
        .await
        .unwrap();
    assert_eq!(result.state, json!({"saved":true}));
    assert_eq!(host.mutations.load(Ordering::SeqCst), 0);
    assert_eq!(services.calls.load(Ordering::SeqCst), 0);
}
