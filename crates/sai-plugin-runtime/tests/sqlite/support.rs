use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::{HttpRequest, HttpResponse, PluginHost},
    Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::sync::Arc;

struct Host;

#[async_trait]
impl PluginHost for Host {
    /// 【数据库快照测试】【网络隔离】测试宿主不提供外部网络
    /// @param request 请求；capabilities 为能力；allow_writes 为写入许可
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP");
    }
}

/// 【数据库快照测试】【实例构造】在零外部能力的只读工具中执行测试正文
/// @param body Lua 工具正文；limits 为显式预算覆盖
/// @returns 独立运行时实例
pub fn plugin(body: &str, limits: Value) -> PluginRuntime {
    let source = format!(
        "sai.register_tool({{name='probe',description='Database snapshot test',parameters={{type='object'}},execute=function(args,ctx) {body} end}})"
    );
    load(&source, limits).unwrap()
}

/// 【数据库快照测试】【原始入口】直接加载给定源码，用于检查初始化及事件边界
/// @param source 完整 Lua 入口；limits 为预算
/// @returns 加载结果，不替调用方吞掉初始化失败
pub fn load(source: &str, limits: Value) -> Result<PluginRuntime> {
    let manifest = PluginManifest::parse(
        &json!({"api_version":1,"id":"sqlite-test","version":"1.0.0","name":"SQLite test",
            "description":"Bounded database snapshots","entry":"init.lua","capabilities":{},"limits":limits}).to_string(),
    ).unwrap();
    PluginRuntime::load(
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap(),
        json!({}),
        Capabilities::default(),
        Arc::new(Host),
    )
}

/// 【数据库快照测试】【可信上下文】固定会话归属并保持只读权限
/// @returns 本次工具的有效上下文
pub fn context() -> InvocationContext {
    InvocationContext {
        session_id: "sqlite-session".into(),
        operation_id: "sqlite-operation".into(),
        workdir: "/sqlite-test".into(),
        ..Default::default()
    }
}

/// 【数据库快照测试】【执行结果】调用实际 Lua 工具并解析完整 JSON
/// @param runtime 实例；args 为工具参数
/// @returns 已解析的结果或完整运行时错误
pub async fn run(runtime: &PluginRuntime, args: Value) -> Result<Value> {
    let output = runtime.call_tool("probe", args, context()).await?;
    Ok(serde_json::from_str(&output)?)
}

pub const SEED: &str = r#"
assert(type(sai.sqlite)=='table','SQLite snapshot API is missing')
local original=sai.sqlite.apply(nil, {
    {op='create_table',name='notes',columns={
        {name='id',kind='integer'},{name='text',kind='text',nullable=false},
        {name='score',kind='real'},{name='optional',kind='text'}},primary_key='id'},
    {op='create_index',name='notes_text',table='notes',columns={'text'}},
    {op='insert',table='notes',rows={
        {id=1,text='第一条',score=1.25,optional=sai.json.null},
        {id=2,text='second',score=-2.5,optional='present'}}}
}, {max_bytes=65536})
"#;
