pub(super) use super::image_support::context;
use super::{image_support::ImageHost, support::FixtureHost};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, PluginRuntime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(super) const TOOL: &str = "search_web_images";

#[derive(Default)]
pub(super) struct WebImageHost {
    pub search: FixtureHost,
    pub images: ImageHost,
    pub fail_write: AtomicBool,
}

impl WebImageHost {
    /// 【搜图测试】【样本宿主】组合搜索文本和二进制替身，避免测试访问真实搜索服务。
    /// @param search 搜索响应序列；images 为图片状态和原始字节序列
    /// @returns 可分别观察查询、下载与保存的宿主
    pub fn new(search: &[(u16, &str)], images: Vec<(u16, Vec<u8>)>) -> Self {
        Self {
            search: FixtureHost::new(search),
            images: ImageHost::new(images),
            ..Default::default()
        }
    }

    /// 【搜图测试】【标准候选】提供有效的主搜索结果，图片下载由调用方指定。
    /// @param candidates 主搜索候选；images 为二进制响应序列
    /// @returns 无须访问 Bing 的宿主
    pub fn candidates(candidates: Vec<Value>, images: Vec<(u16, Vec<u8>)>) -> Self {
        Self::new(
            &[
                (200, "vqd='1-2-3'"),
                (200, &json!({"results":candidates}).to_string()),
            ],
            images,
        )
    }
}

#[async_trait]
impl PluginHost for WebImageHost {
    /// 【搜图测试】【搜索请求】复用文本来源授权检查与请求记录。
    /// @param request 请求；capabilities 为来源授权；allow_writes 为权限
    /// @returns 排队的搜索响应
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        self.search.http(request, capabilities, allow_writes).await
    }

    /// 【搜图测试】【下载请求】复用匿名下载记录，不继承搜索请求的头字段。
    /// @param request 下载请求；capabilities 为有效授权
    /// @returns 排队的原始图片响应
    async fn download_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
    ) -> Result<BinaryResponse> {
        self.images.download_binary(request, capabilities).await
    }

    /// 【搜图测试】【文件保存】复用真实参数检查，可模拟磁盘写入失败。
    /// @param path 输出路径；data 为图片租约；context 为可信目录；capabilities 为写入授权
    /// @returns 已保存元数据或确定的写入错误
    async fn write_binary(
        &self,
        path: String,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryFile> {
        if self.fail_write.load(Ordering::SeqCst) {
            bail!("fixture disk write failed");
        }
        self.images
            .write_binary(path, data, context, capabilities)
            .await
    }
}

/// 【搜图测试】【默认设置】关闭可选视觉与展示，缓存固定在测试目录内。
/// @returns 可按单个测试覆盖的插件设置
pub(super) fn settings() -> Value {
    json!({"cache_dir":"output","auto_preview":false,"vision_screening_enabled":false,"language":"en"})
}

/// 【搜图测试】【完整包】加载正式发布源码和设置兼容层。
/// @param settings 显式设置；host 为可观察的宿主
/// @returns 真实 Lua 运行时
pub(super) fn runtime(settings: Value, host: Arc<dyn PluginHost>) -> PluginRuntime {
    super::image_support::runtime("web-images", settings, host)
}

/// 【搜图测试】【引擎候选】提供相同评分的稳定顺序候选。
/// @param index 候选唯一序号
/// @returns DuckDuckGo 原始候选字段
pub(super) fn candidate(index: usize) -> Value {
    json!({"title":"mountain landscape","url":format!("https://page.test/{index}"),
        "image":format!("https://image.test/{index}"),"thumbnail":"","width":1024,"height":768})
}

/// 【搜图测试】【图片字节】构造具有 PNG 头和可识别尺寸的固定二进制样本。
/// @param width 宽度；height 为高度；marker 为区分图片摘要的额外字节
/// @returns 只用于元数据测试的字节，不发送给真实模型
pub(super) fn png(width: u32, height: u32, marker: u8) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.push(marker);
    bytes
}

/// 【搜图测试】【Bing 样本】编码真实 m 属性，使同一候选可经过第二来源解析。
/// @param candidates DuckDuckGo 风格候选
/// @returns HTML 搜索响应
pub(super) fn bing(candidates: &[Value]) -> String {
    candidates
        .iter()
        .map(|item| {
            let data = json!({"t":item["title"],"purl":item["url"],"murl":item["image"],
            "turl":item["thumbnail"],"w":item["width"],"h":item["height"]});
            format!(
                "<a class=\"iusc\" m=\"{}\"></a>",
                data.to_string()
                    .replace('&', "&amp;")
                    .replace('"', "&quot;")
            )
        })
        .collect()
}
