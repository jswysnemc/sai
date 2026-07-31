use super::{prefers_codex_responses_shape, OpenAiCompatibleClient};
use crate::config::ProviderConfig;
use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProviderProtocol {
    Auto,
    OpenAiChat,
    OpenAiResponses,
    Anthropic,
}

impl ProviderProtocol {
    /// 根据供应商配置解析请求协议。
    ///
    /// 参数:
    /// - `provider`: 当前供应商配置
    ///
    /// 返回:
    /// - 解析后的协议；不支持的配置返回错误
    pub(super) fn from_provider(provider: &ProviderConfig) -> Result<Self> {
        match provider.protocol.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "openai-chat" => Ok(Self::OpenAiChat),
            "openai-responses" => Ok(Self::OpenAiResponses),
            "anthropic" | "anthropic-messages" | "messages" | "claude" | "claude-code"
            | "claude-messages" => Ok(Self::Anthropic),
            protocol => bail!("unsupported provider protocol: {protocol}"),
        }
    }
}

impl OpenAiCompatibleClient {
    /// 判断当前模型是否应使用 OpenAI Responses 协议。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 模型或供应商配置偏好 Responses 协议时返回 true
    pub(super) fn uses_openai_responses(&self) -> bool {
        let model = self.provider.default_model.to_ascii_lowercase();
        model.starts_with("gpt-5")
            || model.contains("codex")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4")
            || prefers_codex_responses_shape(
                &self.provider.default_model,
                &self.provider.base_url,
                &self.provider.client_style,
            )
    }
}
