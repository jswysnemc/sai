use crate::Capabilities;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) mod binary;
mod private;
mod services;
mod system;
mod vision;
pub use binary::{BinaryData, BinaryFile, BinaryResponse, DisplayedImage};
pub use private::{
    validate_storage_key, validate_workspace_path, ArchiveRequest, PluginWorkspace, StorageRequest,
};
pub use services::{
    HostTool, InvocationServices, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelToolCall, ModelUsage,
};
pub use system::{
    DirectoryEntry, DirectoryListing, FileInfo, FileReadRequest, FileText, ProcessOutput,
    ProcessRequest, SystemContext,
};
pub use vision::{VisionModelInfo, VisionRequest, VisionResponse, MAX_VISION_IMAGE_BYTES};

/// 【插件】【HTTP 请求】宿主执行的请求，不允许插件直接创建网络连接。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HttpRequest {
    pub url: String,
    #[serde(default = "get_method")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default = "response_limit")]
    pub max_bytes: usize,
    #[serde(default = "request_timeout")]
    pub timeout_ms: u64,
}

/// 【插件】【HTTP 响应】宿主已完成解码且大小受限的结果。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub text: String,
}

/// 【插件】【宿主接口】插件运行时所需的异步能力，实现不依赖 Sai 应用配置。
#[async_trait]
pub trait PluginHost: Send + Sync {
    /// 【插件二进制】【原始请求】执行精确来源授权的请求，保留响应正文原始字节。
    /// @param request 请求；capabilities 为有效授权；allow_writes 为宿主权限
    /// @returns 有界响应，默认宿主不提供该能力
    async fn http_binary(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<BinaryResponse> {
        anyhow::bail!("binary HTTP is unavailable in this host")
    }

    /// 【插件二进制】【匿名下载】从已授权来源或明确授权的公开网络执行无凭据 GET。
    /// @param request 无正文和自定义头的 GET；capabilities 为有效网络授权
    /// @returns 有界二进制响应，公开网络必须由宿主验证和固定解析地址
    async fn download_binary(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
    ) -> Result<BinaryResponse> {
        anyhow::bail!("binary download is unavailable in this host")
    }

    /// 【插件二进制】【文件输出】在授权目录内写入缓冲，参数不能覆盖宿主权限。
    /// @param path 目标；data 为持有预算的缓冲；context 为可信目录；capabilities 为授权
    /// @returns 实际输出路径与字节数
    async fn write_binary(
        &self,
        _path: String,
        _data: BinaryData,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<BinaryFile> {
        anyhow::bail!("binary file writing is unavailable in this host")
    }

    /// 【插件图片】【终端尺寸】返回当前交互终端的单元格尺寸。
    /// @returns 没有交互终端时返回 None，不开放其他系统状态
    fn terminal_size(&self) -> Option<(u16, u16)> {
        None
    }

    /// 【插件图片】【显示接口】绘制指定图片，图像内容不通过这个接口返回 Lua。
    /// @param path 图片路径；size 为有界显示尺寸；context 为可信目录；capabilities 为展示授权
    /// @returns 实际使用的图片路径
    async fn display_image(
        &self,
        _path: String,
        _size: Option<String>,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<DisplayedImage> {
        anyhow::bail!("image display is unavailable in this host")
    }

    /// 【插件】【私有状态】读取或原子更新宿主会话内的插件数据。
    /// @param request 键值操作；session 为可信会话；capabilities 为有效授权
    /// @returns 读取值、写入后的值或比较交换是否成功
    fn storage(
        &self,
        _request: StorageRequest,
        _session: &str,
        _capabilities: &Capabilities,
    ) -> Result<serde_json::Value> {
        anyhow::bail!("private storage is unavailable in this host")
    }

    /// 【插件】【工作目录创建】重建指定键的私有缓存目录，锁住目录直到调用结束。
    /// @param key 插件私有键；session 为可信会话；capabilities 为有效授权
    /// @returns 不能访问其他工作目录的宿主句柄
    fn workspace(
        &self,
        _key: &str,
        _session: &str,
        _capabilities: &Capabilities,
    ) -> Result<std::sync::Arc<dyn PluginWorkspace>> {
        anyhow::bail!("private workspaces are unavailable in this host")
    }

    /// 【插件】【环境读取】读取精确授权的环境变量，缺失值返回 None。
    /// @param name 变量名；capabilities 为有效授权
    /// @returns 环境变量文本，未实现该能力的宿主返回错误
    fn environment(&self, _name: &str, _capabilities: &Capabilities) -> Result<Option<String>> {
        anyhow::bail!("environment access is unavailable in this host")
    }

    /// 【插件】【文件读取】读取已授权范围内的有界文本。
    /// @param request 路径与字节限制；context 为可信工作目录；capabilities 为授权
    /// @returns 文本与截断信息
    async fn read_text(
        &self,
        _request: FileReadRequest,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<FileText> {
        anyhow::bail!("file reading is unavailable in this host")
    }

    /// 【插件】【目录读取】列出已授权目录中的有界条目。
    /// @param path 路径；max_entries 为条数上限；context 为可信目录；capabilities 为授权
    /// @returns 条目与截断信息
    async fn read_directory(
        &self,
        _path: String,
        _max_entries: usize,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<DirectoryListing> {
        anyhow::bail!("directory reading is unavailable in this host")
    }

    /// 【插件】【属性读取】检查已授权路径，不将未授权路径伪装为不存在。
    /// @param path 路径；context 为可信目录；capabilities 为授权
    /// @returns 文件属性，已授权但不存在时返回 None
    async fn file_info(
        &self,
        _path: String,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        anyhow::bail!("file information is unavailable in this host")
    }

    /// 【插件】【进程执行】按完整模板执行程序并管理超时、取消和有界输出。
    /// @param request 模板与参数；context 为真实目录和权限；capabilities 为有效授权
    /// @returns 退出状态与输出，无法启动或未授权时返回错误
    async fn process(
        &self,
        _request: ProcessRequest,
        _context: SystemContext,
        _capabilities: Capabilities,
    ) -> Result<ProcessOutput> {
        anyhow::bail!("process execution is unavailable in this host")
    }

    /// 【插件】【文本分词】由宿主提供统一的文本 token 估算。
    /// @param text 已通过字节限制的文本
    /// @returns 估算数量；未提供分词能力的宿主返回明确错误
    fn estimate_tokens(&self, _text: &str) -> Result<u64> {
        anyhow::bail!("token estimation is unavailable in this host")
    }

    /// 【插件】【HTTP 执行】执行已经过来源和权限校验的请求。
    /// @param request 请求信息；capabilities 为有效网络授权；allow_writes 为宿主确认的写入权限
    /// @returns 有界响应，失败时返回不包含请求凭据的错误
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse>;
}

/// 返回 HTTP 默认方法，无参数。
fn get_method() -> String {
    "GET".to_string()
}

/// 返回 HTTP 默认响应字节上限，无参数。
fn response_limit() -> usize {
    1024 * 1024
}

/// 【插件】【请求超时】返回单次 HTTP 请求的默认毫秒上限，无参数。
/// @returns 30 秒对应的毫秒数
fn request_timeout() -> u64 {
    30_000
}
