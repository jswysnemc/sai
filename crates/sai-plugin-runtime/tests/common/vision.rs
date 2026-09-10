#![allow(dead_code)]

use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, InvocationContext, PluginRuntime};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::Notify;

#[derive(Default)]
pub struct Services {
    pub text: crate::common::services::RecordingServices,
    pub requests: Mutex<Vec<(VisionRequest, Vec<u8>, usize)>>,
    pub info_calls: AtomicUsize,
    pub disabled: AtomicBool,
    pub block: AtomicBool,
    pub active: AtomicUsize,
    pub response: Mutex<String>,
    pub entered: Notify,
    pub released: Notify,
}

struct Flight<'a>(&'a Services);

impl Drop for Flight<'_> {
    /// 【视觉测试】【请求释放】在成功、超时和取消时记录宿主 Future 已退出。
    /// @returns 无
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.released.notify_one();
    }
}

#[async_trait]
impl InvocationServices for Services {
    /// 【视觉测试】【模型信息】返回公开标识，记录未授权调用是否错误抵达宿主。
    /// @returns 启用时的模型标识，关闭时返回 None
    fn vision_info(&self) -> Result<Option<VisionModelInfo>> {
        self.info_calls.fetch_add(1, Ordering::SeqCst);
        Ok(
            (!self.disabled.load(Ordering::SeqCst)).then(|| VisionModelInfo {
                provider_id: "vision-fixture".into(),
                model: "vision-model".into(),
            }),
        )
    }

    /// 【视觉测试】【图片请求】记录实际字节并支持暂停，以验证取消和租约生命周期。
    /// @param request 视觉请求；image 为受预算保护的图片；max_bytes 为文字上限
    /// @returns 固定正文及可信模型标识
    async fn analyze_image(
        &self,
        request: VisionRequest,
        image: BinaryData,
        max_bytes: usize,
    ) -> Result<VisionResponse> {
        self.requests
            .lock()
            .unwrap()
            .push((request, image.bytes().to_vec(), max_bytes));
        self.active.fetch_add(1, Ordering::SeqCst);
        let _flight = Flight(self);
        self.entered.notify_one();
        if self.block.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        drop(image);
        Ok(VisionResponse {
            content: self.response.lock().unwrap().clone(),
            provider_id: "vision-fixture".into(),
            model: "vision-model".into(),
        })
    }

    /// 【视觉测试】【工具目录】复用已有宿主工具测试替身。
    /// @returns 固定工具目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        self.text.tools()
    }

    /// 【视觉测试】【工具调用】复用已有的调用记录和权限断言。
    /// @param name 工具名；arguments 为 JSON 参数
    /// @returns 测试工具结果
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        self.text.call_tool(name, arguments).await
    }

    /// 【视觉测试】【文本模型】复用文本请求记录，检查文本与视觉共用次数预算。
    /// @param request 文本请求；max_bytes 为输出限制
    /// @returns 固定模型结果
    async fn complete(&self, request: ModelRequest, max_bytes: usize) -> Result<ModelResponse> {
        self.text.complete(request, max_bytes).await
    }
}

/// 【视觉测试】【能力声明】分别声明文本和视觉模型能力，不授予网络或文件能力。
/// @returns 可在各测试中进一步撤销的声明
pub fn capabilities() -> Capabilities {
    Capabilities {
        model: true,
        vision: true,
        ..Default::default()
    }
}

/// 【视觉测试】【真实运行时】加载自包含脚本，通过真实授权交集绑定服务。
/// @param source 脚本；granted 为实际授权；change 为限制调整函数
/// @returns 可调用的 Lua 插件
pub fn runtime(
    source: &str,
    granted: Capabilities,
    change: impl FnOnce(&mut sai_plugin_runtime::ExecutionLimits),
) -> PluginRuntime {
    crate::common::services::runtime(source, capabilities(), granted, change)
}

/// 【视觉测试】【调用上下文】绑定单次服务，默认使用只读工具权限。
/// @param services 可观察的视觉服务
/// @returns 本次可信上下文
pub fn context(services: Arc<Services>) -> InvocationContext {
    InvocationContext {
        services: Some(services),
        ..Default::default()
    }
}
