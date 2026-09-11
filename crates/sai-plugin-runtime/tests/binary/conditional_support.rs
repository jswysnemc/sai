#![allow(dead_code)]

use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, ExecutionLimits, InvocationContext, PluginManifest, PluginPackage,
    PluginRuntime,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub struct WriteCall {
    pub request: BinaryConditionalWrite,
    pub bytes: Vec<u8>,
    pub context: SystemContext,
    pub capabilities: Capabilities,
}

#[derive(Default)]
pub struct Host {
    pub writes: Mutex<Vec<WriteCall>>,
    pub fail: AtomicBool,
    pub matched: AtomicBool,
}

#[async_trait]
impl PluginHost for Host {
    /// 【条件写入测试】【网络隔离】条件写入不得使用网络
    /// @param request 请求；capabilities 为授权；allow_writes 为可信权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【条件写入测试】【宿主记录】保留条件、原始字节和可信调用边界
    /// @param request 条件；data 为数据租约；context 为可信目录；capabilities 为授权交集
    /// @returns 测试指定的匹配状态或错误
    async fn write_binary_if(
        &self,
        request: BinaryConditionalWrite,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<bool> {
        self.writes.lock().unwrap().push(WriteCall {
            request,
            bytes: data.bytes().to_vec(),
            context,
            capabilities,
        });
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixture conditional write failure");
        }
        Ok(self.matched.load(Ordering::SeqCst))
    }
}

/// 【条件写入测试】【目录授权】创建同一路径的读取与输出声明
/// @returns 不包含网络或私有存储能力的授权集合
pub fn capabilities() -> Capabilities {
    serde_json::from_value(
        json!({"system":{"read_paths":["output"]},"binary":{"write_paths":["output"]}}),
    )
    .unwrap()
}

/// 【条件写入测试】【实例加载】允许分别设置声明、授权和执行限制
/// @param source 入口脚本；host 为宿主；declared 为声明；granted 为授权；change 为限制调整
/// @returns 完整运行时或初始化错误
pub fn load(
    source: &str,
    host: Arc<dyn PluginHost>,
    declared: Capabilities,
    granted: Capabilities,
    change: impl FnOnce(&mut ExecutionLimits),
) -> Result<PluginRuntime> {
    let mut manifest = PluginManifest::parse(&json!({
        "api_version":1,"id":"binary-conditional","version":"1.0.0","name":"Conditional tests",
        "description":"Conditional file write contract","entry":"init.lua","capabilities":declared,
    }).to_string())?;
    change(&mut manifest.limits);
    let package = PluginPackage::new(manifest, [("init.lua".into(), source.into())].into())?;
    PluginRuntime::load(package, json!({}), granted, host)
}

/// 【条件写入测试】【默认实例】以相同读写声明和授权加载入口
/// @param source 脚本；host 为宿主；change 为资源限制调整
/// @returns 可执行运行时
pub fn runtime(
    source: &str,
    host: Arc<dyn PluginHost>,
    change: impl FnOnce(&mut ExecutionLimits),
) -> PluginRuntime {
    load(source, host, capabilities(), capabilities(), change).unwrap()
}

/// 【条件写入测试】【可信上下文】固定宿主工作目录并指定是否允许写入
/// @param writable 本次是否允许产生写入
/// @returns 不受 Lua 字段修改影响的调用上下文
pub fn context(writable: bool) -> InvocationContext {
    InvocationContext {
        workdir: "/trusted".into(),
        allow_writes: writable,
        ..Default::default()
    }
}

pub const SOURCE: &str = r#"
    --- 【条件写入测试】【请求转交】使用原始字节入口，并尝试修改可见上下文字段
    --- @param args table 路径、字节及预期摘要
    --- @param ctx table 可见上下文
    --- @return boolean 宿主比较结果
    local function run(args,ctx)
        ctx.workdir="/forged"; ctx.allow_writes=true
        return sai.binary.from_bytes(args.data or "new"):write_if(args.path or "output/index.json",args.expected)
    end
    sai.register_tool({name="run",description="Conditional write",access="optional_writes",parameters={type="object"},execute=run})
"#;
