use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{
    BinaryData, BinaryFile, BinaryResponse, DirectoryListing, DisplayedImage, FileInfo,
    FileReadRequest, FileText, HttpRequest, HttpResponse, PluginHost, ProcessOutput,
    ProcessRequest, SystemContext,
};
use sai_plugin_runtime::Capabilities;

/// 【插件】【Sai 宿主】执行跨平台 HTTP 请求，重定向和响应大小继续受插件授权约束。
pub(super) struct SaiPluginHost;

#[async_trait]
impl PluginHost for SaiPluginHost {
    /// 【插件宿主】【二进制请求】复用精确来源授权和独立大文件时限。
    /// @param request 请求；capabilities 为授权；allow_writes 为可信写入权限
    /// @returns 原始有界响应
    async fn http_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<BinaryResponse> {
        super::binary::request(request, capabilities, allow_writes).await
    }

    /// 【插件宿主】【匿名下载】下载不带凭据的公开或精确授权来源。
    /// @param request 下载请求；capabilities 为授权
    /// @returns 原始有界响应
    async fn download_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
    ) -> Result<BinaryResponse> {
        super::binary::download(request, capabilities).await
    }

    /// 【插件宿主】【文件输出】通过目录句柄写入并原子发布。
    /// @param path 目标；data 为缓冲租约；context 为可信目录；capabilities 为写入授权
    /// @returns 实际路径和字节数
    async fn write_binary(
        &self,
        path: String,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryFile> {
        super::binary::write(path, data, context, capabilities).await
    }

    /// 【插件宿主】【终端尺寸】获取当前终端单元格数量。
    /// @returns 有可用终端时返回宽高
    fn terminal_size(&self) -> Option<(u16, u16)> {
        crossterm::terminal::size().ok()
    }

    /// 【插件宿主】【图片显示】将有界图片交给公共终端渲染器。
    /// @param path 路径；size 为单元格尺寸；context 为可信目录；capabilities 为展示授权
    /// @returns 已显示的原图片路径
    async fn display_image(
        &self,
        path: String,
        size: Option<String>,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DisplayedImage> {
        super::binary::display(path, size, context, capabilities).await
    }
    /// 【插件】【环境能力】读取当前插件明确授权的环境变量。
    /// @param name 变量名；capabilities 为有效授权
    /// @returns 环境值或 None
    fn environment(&self, name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
        super::system::environment(name, capabilities)
    }

    /// 【插件】【文本能力】通过授权目录句柄读取有界文件文本。
    /// @param request 读取请求；context 为可信目录；capabilities 为授权
    /// @returns 文本与截断信息
    async fn read_text(
        &self,
        request: FileReadRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<FileText> {
        super::system::read_text(request, context, capabilities).await
    }

    /// 【插件】【目录能力】列出授权目录中的有界条目。
    /// @param path 目录；max_entries 为条数；context 为可信目录；capabilities 为授权
    /// @returns 目录条目与截断信息
    async fn read_directory(
        &self,
        path: String,
        max_entries: usize,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DirectoryListing> {
        super::system::read_directory(path, max_entries, context, capabilities).await
    }

    /// 【插件】【属性能力】读取授权路径的通用属性。
    /// @param path 路径；context 为可信目录；capabilities 为授权
    /// @returns 属性或不存在
    async fn file_info(
        &self,
        path: String,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        super::system::file_info(path, context, capabilities).await
    }

    /// 【插件】【进程能力】执行授权模板，取消时由资源守卫清理进程树。
    /// @param request 模板与选项；context 为可信目录及权限；capabilities 为授权
    /// @returns 进程输出及退出状态
    async fn process(
        &self,
        request: ProcessRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<ProcessOutput> {
        super::system::execute(request, context, capabilities).await
    }

    /// 【插件】【文本分词】复用主会话分词器，避免业务插件自行实现近似规则。
    /// @param text 有界文本
    /// @returns 统一估算的 token 数量
    fn estimate_tokens(&self, text: &str) -> Result<u64> {
        Ok(crate::token_estimate::estimate_tokens(text) as u64)
    }

    /// 【插件】【HTTP 执行】把已授权请求交给统一网络执行模块。
    /// @param request 已校验请求；capabilities 为有效网络授权；allow_writes 为宿主调用权限
    /// @returns UTF-8 解码后的有界响应，不把 URL 凭据写入错误
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        super::http::execute(request, capabilities, allow_writes).await
    }
}
