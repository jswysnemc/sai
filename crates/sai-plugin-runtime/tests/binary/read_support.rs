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

pub struct ReadCall {
    pub path: String,
    pub max_bytes: usize,
    pub context: SystemContext,
    pub capabilities: Capabilities,
}

#[derive(Default)]
pub struct Host {
    pub body: Mutex<Vec<u8>>,
    pub reads: Mutex<Vec<ReadCall>>,
    pub writes: Mutex<Vec<(String, Vec<u8>, SystemContext)>>,
    pub fail: AtomicBool,
    pub retain: AtomicBool,
    pub retained: Mutex<Vec<BinaryData>>,
    pub replacement: Mutex<Option<BinaryData>>,
}

#[async_trait]
impl PluginHost for Host {
    /// 【二进制读取测试】【网络隔离】本地读取不得退回网络接口
    /// @param request 请求；capabilities 为授权；allow_writes 为可信权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP request")
    }

    /// 【二进制读取测试】【读取记录】记录受限缓冲和可信上下文，支持失败及错误宿主结果
    /// @param path 路径；buffer 为预留预算；context 为可信目录；capabilities 为有效授权
    /// @returns 固定原始字节或测试指定结果
    async fn read_binary(
        &self,
        path: String,
        mut buffer: BinaryReadBuffer,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryData> {
        capabilities.system.check_read_request(&path)?;
        self.reads.lock().unwrap().push(ReadCall {
            path,
            max_bytes: buffer.max_bytes(),
            context,
            capabilities,
        });
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixture read failure");
        }
        if let Some(data) = self.replacement.lock().unwrap().take() {
            return Ok(data);
        }
        buffer.extend_from_slice(&self.body.lock().unwrap())?;
        let data = buffer.finish();
        if self.retain.load(Ordering::SeqCst) {
            self.retained.lock().unwrap().push(data.clone());
        }
        Ok(data)
    }

    /// 【二进制读取测试】【写入组合】记录读取缓冲交给写入接口后的完整原始字节
    /// @param path 输出；data 为预算租约；context 为可信目录；capabilities 为写入授权
    /// @returns 文件元数据
    async fn write_binary(
        &self,
        path: String,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryFile> {
        capabilities
            .binary
            .check_write(&path, context.allow_writes)?;
        let bytes = data.bytes().len();
        self.writes
            .lock()
            .unwrap()
            .push((path.clone(), data.bytes().to_vec(), context));
        Ok(BinaryFile { path, bytes })
    }
}

/// 【二进制读取测试】【读取声明】仅声明一个目录的读取权限
/// @returns 不包含网络、写入或模型授权的能力集合
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"read_paths":["allowed"]}})).unwrap()
}

/// 【二进制读取测试】【加载入口】按独立声明和授权加载脚本，允许检查初始化错误
/// @param source 脚本；host 为宿主；declared 为声明；granted 为授权；change 为限制调整
/// @returns 完整运行时或加载错误
pub fn load(
    source: &str,
    host: Arc<dyn PluginHost>,
    declared: Capabilities,
    granted: Capabilities,
    change: impl FnOnce(&mut ExecutionLimits),
) -> Result<PluginRuntime> {
    let mut manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"binary-read","version":"1.0.0","name":"Read tests",
            "description":"Binary file read contract","entry":"init.lua","capabilities":declared,
        })
        .to_string(),
    )?;
    change(&mut manifest.limits);
    let package = PluginPackage::new(manifest, [("init.lua".into(), source.into())].into())?;
    PluginRuntime::load(package, json!({}), granted, host)
}

/// 【二进制读取测试】【默认运行时】使用相同读取声明和授权加载测试脚本
/// @param source 脚本；host 为宿主；change 为资源限制调整
/// @returns 可执行运行时
pub fn runtime(
    source: &str,
    host: Arc<dyn PluginHost>,
    change: impl FnOnce(&mut ExecutionLimits),
) -> PluginRuntime {
    load(source, host, capabilities(), capabilities(), change).unwrap()
}

/// 【二进制读取测试】【可信目录】绑定只读上下文，拒绝脚本伪造同名字段
/// @returns 固定宿主目录下的只读调用上下文
pub fn context() -> InvocationContext {
    InvocationContext {
        workdir: "/trusted".into(),
        ..Default::default()
    }
}

pub const SOURCE: &str = r#"
    --- 【二进制读取测试】【缓冲读取】返回长度，保留可观察的调用边界
    --- @param args table 可选读取上限及时限
    --- @return integer 原始字节数
    local function read(args)
        return sai.binary.read_file("allowed/image.bin", args.options):len()
    end
    sai.register_tool({name="read",description="Read binary file",parameters={type="object"},execute=read})
"#;
