use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, InvocationContext, PluginRuntime, ToolAccess};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
pub(super) struct ImageHost {
    pub responses: Mutex<VecDeque<BinaryResponse>>,
    pub requests: Mutex<Vec<HttpRequest>>,
    pub downloads: Mutex<Vec<HttpRequest>>,
    pub writes: Mutex<Vec<(String, Vec<u8>)>>,
    pub displays: Mutex<Vec<(String, Option<String>)>>,
    pub terminal: Mutex<Option<(u16, u16)>>,
    pub fail_display: AtomicBool,
}

impl ImageHost {
    /// 【图片测试】【响应样本】按消费顺序装入固定二进制响应。
    /// @param responses 状态码与原始正文
    /// @returns 不访问外网的宿主
    pub fn new(responses: Vec<(u16, Vec<u8>)>) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|(status, body)| BinaryResponse {
                        status,
                        headers: Default::default(),
                        body,
                    })
                    .collect(),
            ),
            ..Default::default()
        }
    }

    /// 【图片测试】【样本消费】返回下一个响应，意外额外请求必须失败。
    /// @returns 固定响应或样本耗尽错误
    fn next(&self) -> Result<BinaryResponse> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("no image fixture"))
    }
}

#[async_trait]
impl PluginHost for ImageHost {
    /// 【图片测试】【文本隔离】生成链路必须使用二进制接口。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 明确错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected text HTTP")
    }

    /// 【图片测试】【生成请求】记录精确授权 API 请求，保留模型与请求正文。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定响应
    async fn http_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<BinaryResponse> {
        capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        self.requests.lock().unwrap().push(request);
        self.next()
    }

    /// 【图片测试】【图片下载】只记录独立匿名请求，不继承生成请求头。
    /// @param request 下载请求；capabilities 为授权
    /// @returns 固定响应
    async fn download_binary(
        &self,
        request: HttpRequest,
        _: Capabilities,
    ) -> Result<BinaryResponse> {
        assert_eq!(request.method, "GET");
        assert!(request.headers.is_empty());
        assert!(request.body.is_none());
        self.downloads.lock().unwrap().push(request);
        self.next()
    }

    /// 【图片测试】【保存记录】捕获解码后的实际字节，路径相对于可信工作目录展开。
    /// @param path 路径；data 为字节租约；context 为可信目录；capabilities 为授权
    /// @returns 保存路径和字节数
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
        let path = std::path::Path::new(&context.workdir)
            .join(path)
            .to_string_lossy()
            .into_owned();
        self.writes
            .lock()
            .unwrap()
            .push((path.clone(), data.bytes().to_vec()));
        Ok(BinaryFile {
            path,
            bytes: data.bytes().len(),
        })
    }

    /// 【图片测试】【尺寸样本】使用固定终端尺寸，不依赖执行测试的终端。
    /// @returns 配置的可选尺寸
    fn terminal_size(&self) -> Option<(u16, u16)> {
        *self.terminal.lock().unwrap()
    }

    /// 【图片测试】【绘制记录】只记录绘制参数，测试中不产生终端协议输出。
    /// @param path 图片路径；size 为尺寸；context 为目录；capabilities 为授权
    /// @returns 展示成功路径或固定错误
    async fn display_image(
        &self,
        path: String,
        size: Option<String>,
        _: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DisplayedImage> {
        assert!(capabilities.binary.display_images);
        self.displays.lock().unwrap().push((path.clone(), size));
        if self.fail_display.load(Ordering::SeqCst) {
            bail!("fixture preview failed");
        }
        Ok(DisplayedImage { path })
    }
}

/// 【图片测试】【实际包加载】加载发布源码及原配置兼容层。
/// @param id 包 ID；settings 为显式设置；host 为可观察宿主
/// @returns 完整真实运行时
pub(super) fn runtime(id: &str, settings: Value, host: Arc<dyn PluginHost>) -> PluginRuntime {
    let mut package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|p| p.manifest.id == id)
        .unwrap();
    let overrides = crate::plugins::compatibility::resolve(
        id,
        &crate::config::AppConfig::default(),
        &settings,
        &package.manifest.capabilities,
    )
    .unwrap()
    .unwrap();
    package.manifest.capabilities = overrides.capabilities.clone();
    PluginRuntime::load(package, overrides.settings, overrides.capabilities, host).unwrap()
}

/// 【图片测试】【默认配置】使用固定非真实凭据和输出目录。
/// @returns 可被个别测试覆盖的生成设置
pub(super) fn generation_settings() -> Value {
    json!({"api_keys":[" "," fixture-key ","unused-key"],"base_url":"https://api.example.test/", "output_dir":"output","auto_print":false})
}

/// 【图片测试】【调用上下文】独立的可写宿主上下文。
/// @returns 可执行生成工具的上下文
pub(super) fn context() -> InvocationContext {
    InvocationContext {
        allow_writes: true,
        workdir: "/fixture".into(),
        ..Default::default()
    }
}

#[derive(Default)]
pub(super) struct PreviewServices {
    pub enabled: bool,
    pub fail: bool,
    pub calls: Mutex<Vec<Value>>,
}

#[async_trait]
impl InvocationServices for PreviewServices {
    /// 【图片测试】【显示目录】模拟当前任务是否公开 print_image。
    /// @returns 可见显示工具或空目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        Ok(if self.enabled {
            vec![HostTool {
                name: "print_image".into(),
                display_name: "Display".into(),
                description: "Display".into(),
                parameters: json!({"type":"object"}),
                access: ToolAccess::ReadOnly,
            }]
        } else {
            vec![]
        })
    }

    /// 【图片测试】【预览调用】记录图片参数，失败不应改变生成结果。
    /// @param name 精确工具名；arguments 为 JSON 参数
    /// @returns 固定成功文本或预览错误
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        assert_eq!(name, "print_image");
        self.calls
            .lock()
            .unwrap()
            .push(serde_json::from_str(arguments)?);
        if self.fail {
            bail!("fixture preview failed");
        }
        Ok("printed image in terminal".into())
    }

    /// 【图片测试】【模型隔离】图片流程不应调用聊天模型。
    /// @param request 请求；max_bytes 为上限
    /// @returns 明确错误
    async fn complete(&self, _: ModelRequest, _: usize) -> Result<ModelResponse> {
        bail!("unexpected model completion")
    }
}
