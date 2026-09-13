#![allow(dead_code)]

use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{BinaryResponse, HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::{
    Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

pub const INPUT: &str =
    "local data=sai.binary.request({url='https://document.test/input',max_bytes=16777216}).body";

struct Host(Vec<u8>);

#[async_trait]
impl PluginHost for Host {
    /// 【文档测试】【文本拦截】测试禁止退回有文本上限的 HTTP 接口
    /// @param request 请求；capabilities 为权限；allow_writes 为写入上下文
    /// @returns 明确的接口选择错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        anyhow::bail!("text HTTP is not part of the document fixture")
    }

    /// 【文档测试】【原始正文】提供大于 Lua 文本上限的原始响应
    /// @param request 请求；capabilities 为权限；allow_writes 为写入上下文
    /// @returns 保持所有输入字节的正文
    async fn http_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<BinaryResponse> {
        capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        anyhow::ensure!(
            self.0.len() <= request.max_bytes,
            "fixture response exceeds byte limit"
        );
        Ok(BinaryResponse {
            status: 200,
            url: request.url,
            headers: BTreeMap::new(),
            body: self.0.clone(),
        })
    }
}

/// 【文档测试】【实例构造】用独立源码、指定限制和原始字节创建实际运行时
/// @param source 完整 Lua 入口；bytes 为 HTTP 原始正文；limits 为资源限制覆盖
/// @returns 加载结果，便于验证初始化阶段拒绝
pub fn load(source: &str, bytes: Vec<u8>, limits: Value) -> Result<PluginRuntime> {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"document-test","version":"1.0.0","name":"Document test",
            "description":"Bounded document conversion","entry":"init.lua",
            "capabilities":{"http":["https://document.test"]},"limits":limits
        })
        .to_string(),
    )?;
    let grants = manifest.capabilities.clone();
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), source.into())]),
    )?;
    PluginRuntime::load(package, json!({}), grants, Arc::new(Host(bytes)))
}

/// 【文档测试】【工具构造】注册只读工具以执行指定正文处理
/// @param body 回调代码；bytes 为原始响应；limits 为资源限制
/// @returns 实际 Lua 运行时
pub fn plugin(body: &str, bytes: Vec<u8>, limits: Value) -> PluginRuntime {
    load(&format!(r#"
        local saved
        --- 【文档测试】【执行入口】运行指定的正文处理场景
        --- @param args table 本次场景参数
        --- @return any 场景结果
        local function run(args)
            {body}
        end
        sai.register_tool({{name='run',description='Document conversion',parameters={{type='object'}},execute=run}})
    "#), bytes, limits).unwrap()
}

/// 【文档测试】【结果解析】执行真实回调并解析完整 JSON
/// @param plugin 实例；args 为输入参数
/// @returns 结构化结果或执行错误
pub async fn run(plugin: &PluginRuntime, args: Value) -> Result<Value> {
    let output = plugin
        .call_tool("run", args, InvocationContext::default())
        .await?;
    Ok(serde_json::from_str(&output)?)
}
