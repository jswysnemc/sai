#![allow(dead_code)]

use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, ExecutionLimits, InvocationContext, PluginManifest, PluginPackage,
    PluginRuntime,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
pub struct Host {
    pub body: Mutex<Vec<u8>>,
    pub requests: Mutex<Vec<HttpRequest>>,
    pub writes: Mutex<Vec<(String, Vec<u8>, SystemContext)>>,
    pub images: Mutex<Vec<(String, Option<String>, SystemContext)>>,
    pub leases: Mutex<Vec<BinaryData>>,
    pub retain_writes: AtomicBool,
    pub block_network: AtomicBool,
    pub active: AtomicUsize,
    pub entered: tokio::sync::Notify,
    pub released: tokio::sync::Notify,
}

struct Active<'a>(&'a Host);
impl Drop for Active<'_> {
    /// 【二进制测试】【网络释放】通知测试当前宿主 Future 已被释放。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

#[async_trait]
impl PluginHost for Host {
    /// 【二进制测试】【文本隔离】测试不应退回普通文本 HTTP。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 明确错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected text request")
    }

    /// 【二进制测试】【请求记录】保留运行时收窄后的请求参数。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定原始响应，或等待取消
    async fn http_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<BinaryResponse> {
        capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        self.requests.lock().unwrap().push(request);
        self.active.fetch_add(1, Ordering::SeqCst);
        let _active = Active(self);
        self.entered.notify_one();
        if self.block_network.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(BinaryResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: self.body.lock().unwrap().clone(),
        })
    }

    /// 【二进制测试】【下载记录】公开下载只用于检查运行时的参数过滤。
    /// @param request 下载请求；capabilities 为授权
    /// @returns 固定响应
    async fn download_binary(
        &self,
        request: HttpRequest,
        _: Capabilities,
    ) -> Result<BinaryResponse> {
        self.requests.lock().unwrap().push(request);
        Ok(BinaryResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: self.body.lock().unwrap().clone(),
        })
    }

    /// 【二进制测试】【写入记录】记录可信目录，可保留缓冲模拟未结束的阻塞 I/O。
    /// @param path 路径；data 为预算租约；context 为可信目录；capabilities 为授权
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
        if self.retain_writes.load(Ordering::SeqCst) {
            self.leases.lock().unwrap().push(data);
        }
        Ok(BinaryFile { path, bytes })
    }

    /// 【二进制测试】【终端尺寸】提供固定终端行列。
    /// @returns 120 列和 40 行
    fn terminal_size(&self) -> Option<(u16, u16)> {
        Some((120, 40))
    }

    /// 【二进制测试】【展示记录】不输出真实图像，只保存调用参数。
    /// @param path 图片；size 为尺寸；context 为可信目录；capabilities 为展示授权
    /// @returns 原路径
    async fn display_image(
        &self,
        path: String,
        size: Option<String>,
        context: SystemContext,
        _: Capabilities,
    ) -> Result<DisplayedImage> {
        self.images
            .lock()
            .unwrap()
            .push((path.clone(), size, context));
        Ok(DisplayedImage { path })
    }
}

/// 【二进制测试】【基础授权】创建可精确请求、匿名下载、输出和展示的完整声明。
/// @returns 测试能力集合
pub fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"http":["https://example.test"],
        "binary":{"public_downloads":true,"write_paths":["output"],"display_images":true}}))
    .unwrap()
}

/// 【二进制测试】【实际运行时】用自包含脚本加载真实 Lua 绑定。
/// @param source 入口脚本；host 为可观察宿主；grants 为有效授权；limits 为限制调整函数
/// @returns 可执行实例
pub fn runtime(
    source: &str,
    host: Arc<Host>,
    grants: Capabilities,
    limits: impl FnOnce(&mut ExecutionLimits),
) -> PluginRuntime {
    let mut manifest = PluginManifest::parse(&json!({"api_version":1,"id":"binary-test","version":"1.0.0",
        "name":"Binary tests","description":"Runtime binary contract","entry":"init.lua","capabilities":capabilities()}).to_string()).unwrap();
    limits(&mut manifest.limits);
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【二进制测试】【可信上下文】使用真实宿主目录标记，不信任 Lua 中的同名字段。
/// @param writable 是否允许写入工具
/// @returns 测试调用上下文
pub fn context(writable: bool) -> InvocationContext {
    InvocationContext {
        workdir: "/trusted".into(),
        allow_writes: writable,
        ..Default::default()
    }
}
