use super::{storage, workspace};
use crate::paths::SaiPaths;
use crate::plugins::host::SaiPluginHost;
use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities};
use std::sync::Arc;

/// 【插件宿主】【私有归属】绑定插件标识和应用根目录，Lua 无法选择其他插件的存储。
pub(in crate::plugins) struct PrivatePluginHost {
    paths: SaiPaths,
    id: String,
    revision: Option<String>,
}

impl PrivatePluginHost {
    /// 【插件宿主】【绑定】构造宿主能力组合，初始化不创建目录或执行网络请求。
    /// @param paths 应用路径；id 为清单中验证后的插件标识
    /// @returns 可传给运行时的独立宿主
    #[cfg(test)]
    pub(in crate::plugins) fn new(paths: &SaiPaths, id: &str) -> Self {
        Self {
            paths: paths.clone(),
            id: id.to_string(),
            revision: None,
        }
    }

    /// 【插件宿主】【调度绑定】把调用实例的源码与授权摘要绑定到持久调度入口。
    /// @param paths 应用路径；id 为插件；revision 为已校验描述符摘要
    /// @returns 具备可信调度归属的宿主
    pub(in crate::plugins) fn with_revision(paths: &SaiPaths, id: &str, revision: String) -> Self {
        Self {
            paths: paths.clone(),
            id: id.into(),
            revision: Some(revision),
        }
    }
}

#[async_trait]
impl PluginHost for PrivatePluginHost {
    /// 【插件宿主】【持久调度】插件身份和源码摘要来自宿主实例，Lua 无法覆盖。
    /// @param request 操作；context 为可信上下文；capabilities 为有效授权
    /// @returns 任务操作结果
    async fn scheduler(
        &self,
        request: SchedulerRequest,
        context: &SystemContext,
        capabilities: &Capabilities,
    ) -> Result<SchedulerResponse> {
        crate::plugins::scheduler::execute(
            &self.paths,
            &self.id,
            self.revision.as_deref(),
            request,
            context,
            capabilities,
        )
        .await
    }
    /// 【插件宿主】【通知代理】直接通知使用可信调用上下文与当前插件有效授权。
    /// @param request 通知请求；context 为可信目录与权限；capabilities 为授权
    /// @returns 投递完成的通道
    async fn notify(
        &self,
        request: NotificationRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<NotificationDelivery> {
        SaiPluginHost.notify(request, context, capabilities).await
    }

    /// 【插件宿主】【二进制请求】复用精确来源授权和独立大文件时限。
    /// @param request 请求；capabilities 为授权；allow_writes 为可信写入权限
    /// @returns 原始有界响应
    async fn http_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<BinaryResponse> {
        SaiPluginHost
            .http_binary(request, capabilities, allow_writes)
            .await
    }

    /// 【插件宿主】【匿名下载】下载不带凭据的公开或精确授权来源。
    /// @param request 下载请求；capabilities 为授权
    /// @returns 原始有界响应
    async fn download_binary(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
    ) -> Result<BinaryResponse> {
        SaiPluginHost.download_binary(request, capabilities).await
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
        SaiPluginHost
            .write_binary(path, data, context, capabilities)
            .await
    }

    /// 【插件宿主】【终端尺寸】获取当前终端单元格数量。
    /// @returns 有可用终端时返回宽高
    fn terminal_size(&self) -> Option<(u16, u16)> {
        SaiPluginHost.terminal_size()
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
        SaiPluginHost
            .display_image(path, size, context, capabilities)
            .await
    }
    /// 【插件宿主】【状态能力】执行绑定插件的会话状态操作。
    /// @param request 键值操作；session 为可信会话；capabilities 为有效授权
    /// @returns 原子操作结果
    fn storage(
        &self,
        request: StorageRequest,
        session: &str,
        capabilities: &Capabilities,
    ) -> Result<serde_json::Value> {
        storage::execute(&self.paths, &self.id, session, request, capabilities)
    }

    /// 【插件宿主】【持久存储】访问绑定插件的独立记录，跨会话写入仍需宿主确认权限。
    /// @param request 键值操作；capabilities 为有效授权；allow_writes 为可信写入权限
    /// @returns 原子读取、写入或比较交换结果
    fn plugin_storage(
        &self,
        request: StorageRequest,
        capabilities: &Capabilities,
        allow_writes: bool,
    ) -> Result<serde_json::Value> {
        storage::execute_plugin(&self.paths, &self.id, request, capabilities, allow_writes)
    }

    /// 【插件宿主】【目录能力】创建绑定插件的私有缓存目录。
    /// @param key 目录键；session 为可信会话；capabilities 为有效授权
    /// @returns 受管目录句柄
    fn workspace(
        &self,
        key: &str,
        session: &str,
        capabilities: &Capabilities,
    ) -> Result<Arc<dyn PluginWorkspace>> {
        workspace::open(&self.paths, &self.id, session, key, capabilities)
    }

    /// 【插件宿主】【环境代理】复用原有环境授权。
    /// @param name 变量；capabilities 为授权
    /// @returns 可选环境值
    fn environment(&self, name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
        SaiPluginHost.environment(name, capabilities)
    }

    /// 【插件宿主】【读取代理】复用原有文件能力。
    /// @param request 请求；context 为可信目录；capabilities 为授权
    /// @returns 文本和截断标记
    async fn read_text(
        &self,
        request: FileReadRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<FileText> {
        SaiPluginHost
            .read_text(request, context, capabilities)
            .await
    }

    /// 【插件宿主】【目录代理】复用原有目录能力。
    /// @param path 路径；max_entries 为上限；context 为可信目录；capabilities 为授权
    /// @returns 目录结果
    async fn read_directory(
        &self,
        path: String,
        max_entries: usize,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DirectoryListing> {
        SaiPluginHost
            .read_directory(path, max_entries, context, capabilities)
            .await
    }

    /// 【插件宿主】【属性代理】复用原有文件属性能力。
    /// @param path 路径；context 为可信目录；capabilities 为授权
    /// @returns 可选文件属性
    async fn file_info(
        &self,
        path: String,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        SaiPluginHost.file_info(path, context, capabilities).await
    }

    /// 【插件宿主】【进程代理】普通模板仍使用可信任务目录。
    /// @param request 模板；context 为可信目录与权限；capabilities 为授权
    /// @returns 进程结果
    async fn process(
        &self,
        request: ProcessRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<ProcessOutput> {
        SaiPluginHost.process(request, context, capabilities).await
    }

    /// 【插件宿主】【分词代理】复用应用分词器。
    /// @param text 正文
    /// @returns token 估算
    fn estimate_tokens(&self, text: &str) -> Result<u64> {
        SaiPluginHost.estimate_tokens(text)
    }

    /// 【插件宿主】【网络代理】复用有界请求与逐次重定向检查。
    /// @param request 请求；capabilities 为授权；allow_writes 为写入权限
    /// @returns 文本响应
    async fn http(
        &self,
        request: HttpRequest,
        capabilities: Capabilities,
        allow_writes: bool,
    ) -> Result<HttpResponse> {
        SaiPluginHost
            .http(request, capabilities, allow_writes)
            .await
    }
}
