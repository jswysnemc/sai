use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

#[derive(Default)]
pub(super) struct MemeModels {
    pub response: Mutex<String>,
    pub requests: Mutex<Vec<ModelRequest>>,
    pub vision_requests: Mutex<Vec<(VisionRequest, Vec<u8>)>>,
    pub fail: AtomicBool,
}

#[async_trait]
impl InvocationServices for MemeModels {
    /// 【表情模型测试】【无工具请求】策略不能让宿主自动执行模型建议
    /// @returns 空工具目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        Ok(Vec::new())
    }

    /// 【表情模型测试】【调用隔离】测试模型不开放额外工具
    /// @param name 名称；arguments 为参数
    /// @returns 明确错误
    async fn call_tool(&self, _: &str, _: &str) -> Result<String> {
        bail!("unexpected tool call")
    }

    /// 【表情模型测试】【固定决策】捕获实际 Lua 提示词，提供可控模型正文
    /// @param request 单次请求；max_bytes 为预算
    /// @returns 固定正文或可控错误
    async fn complete(&self, request: ModelRequest, _: usize) -> Result<ModelResponse> {
        assert!(request.tools.is_empty());
        self.requests.lock().unwrap().push(request);
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixture model failure");
        }
        Ok(ModelResponse {
            content: self.response.lock().unwrap().clone(),
            ..Default::default()
        })
    }

    /// 【表情模型测试】【固定视觉】保留发送的字节、媒体类型和提示词
    /// @param request 视觉请求；image 为原图片；max_bytes 为预算
    /// @returns 固定视觉正文或明确错误
    async fn analyze_image(
        &self,
        request: VisionRequest,
        image: BinaryData,
        _: usize,
    ) -> Result<VisionResponse> {
        self.vision_requests
            .lock()
            .unwrap()
            .push((request, image.bytes().to_vec()));
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixture vision failure");
        }
        Ok(VisionResponse {
            content: self.response.lock().unwrap().clone(),
            provider_id: "fixture".into(),
            model: "fixture-vision".into(),
        })
    }
}
