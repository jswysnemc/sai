use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use sai_plugin_runtime::Capabilities;

/// 【插件】【Sai 宿主】执行跨平台 HTTP 请求，重定向和响应大小继续受插件授权约束。
pub(super) struct SaiPluginHost;

#[async_trait]
impl PluginHost for SaiPluginHost {
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
