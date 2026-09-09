use crate::Capabilities;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod services;
pub use services::{
    HostTool, InvocationServices, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelToolCall, ModelUsage,
};

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
