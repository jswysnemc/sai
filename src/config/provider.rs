use super::defaults::*;
use super::model::*;
use crate::default_models::{
    OPENCODE_DEFAULT_CHAT_MODEL, OPENCODE_PROVIDER_ID, OPENCODE_ZEN_BASE_URL,
};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;

impl ProviderConfig {
    /// 判断当前配置是否指向官方 Anthropic API。
    ///
    /// 返回:
    /// - API 主机为 `api.anthropic.com` 时返回 true
    pub fn uses_official_anthropic_api(&self) -> bool {
        reqwest::Url::parse(&self.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .is_some_and(|host| host.eq_ignore_ascii_case("api.anthropic.com"))
    }

    pub fn default_opencodezen() -> Self {
        Self {
            id: OPENCODE_PROVIDER_ID.to_string(),
            display_name: "opencode Zen".to_string(),
            base_url: OPENCODE_ZEN_BASE_URL.to_string(),
            protocol: default_provider_protocol(),
            api_key: None,
            models: vec![OPENCODE_DEFAULT_CHAT_MODEL.to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: OPENCODE_DEFAULT_CHAT_MODEL.to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
        }
    }

    pub fn default_openai() -> Self {
        Self {
            id: "openai".to_string(),
            display_name: "OpenAI-compatible".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            protocol: default_provider_protocol(),
            api_key: Some("$env:OPENAI_API_KEY".to_string()),
            models: vec!["gpt-4o-mini".to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: "gpt-4o-mini".to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
        }
    }

    /// 创建官方 Anthropic Messages 供应商模板。
    ///
    /// 返回:
    /// - 使用官方 API 地址和 Claude 默认模型的配置
    pub fn default_anthropic() -> Self {
        Self {
            id: "anthropic".to_string(),
            display_name: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            protocol: "anthropic".to_string(),
            api_key: Some("$env:ANTHROPIC_API_KEY".to_string()),
            models: vec!["claude-sonnet-4-5".to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: "claude-sonnet-4-5".to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
        }
    }

    pub fn default_templates() -> Vec<Self> {
        let mut providers = vec![Self::default_opencodezen()];
        providers.extend([
            Self::template("openai", "OpenAI", "https://api.openai.com/v1"),
            Self::default_anthropic(),
            Self::template("deepseek", "DeepSeek", "https://api.deepseek.com"),
            Self::template(
                "gemini",
                "Gemini",
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            Self::template(
                "xiaomi",
                "Xiaomi",
                "https://token-plan-sgp.xiaomimimo.com/v1",
            ),
            Self::template("minimax", "Minimax", "https://api.minimaxi.com/v1"),
            Self::template("openrouter", "OpenRouter", "https://openrouter.ai/api/v1"),
            Self::template("ollama", "Ollama", "http://localhost:11434/v1"),
            Self::template("lmstudio", "LMStudio", "http://localhost:1234/v1"),
        ]);
        providers
    }

    fn template(id: &str, display_name: &str, base_url: &str) -> Self {
        Self {
            id: id.to_string(),
            display_name: display_name.to_string(),
            base_url: base_url.to_string(),
            protocol: default_provider_protocol(),
            api_key: None,
            models: Vec::new(),
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: String::new(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
        }
    }

    pub fn new_openai_compatible() -> Self {
        let mut provider = Self::default_openai();
        provider.models.clear();
        provider.default_model.clear();
        provider
    }

    /// 解析本供应商 HTTP 请求使用的 User-Agent。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 自定义 UA；未配置时 Codex 模式返回 Codex CLI UA，否则返回 sai 默认 UA
    pub fn effective_user_agent(&self) -> String {
        let custom = self.user_agent.trim();
        if !custom.is_empty() {
            return custom.to_string();
        }
        if self.client_style.trim().eq_ignore_ascii_case("codex") {
            return super::defaults::CODEX_CLI_USER_AGENT.to_string();
        }
        super::defaults::DEFAULT_HTTP_USER_AGENT.to_string()
    }

    pub fn resolved_api_key(&self, paths: &SaiPaths) -> Result<String> {
        if let Some(api_key) = self.api_key.as_deref() {
            if let Some(env_name) = api_key.strip_prefix("$env:") {
                return std::env::var(env_name)
                    .with_context(|| format!("environment variable {env_name} is not set"));
            }
            if !api_key.is_empty() {
                return Ok(api_key.to_string());
            }
        }

        let secrets = SecretsConfig::load(paths)?;
        if let Some(api_key) = secrets
            .api_keys
            .get(&self.id)
            .cloned()
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(api_key);
        }

        if self.is_opencode_zen() {
            return Ok("public".to_string());
        }

        bail!("missing API key for provider {}", self.id)
    }

    pub fn is_opencode_zen(&self) -> bool {
        matches!(self.id.as_str(), OPENCODE_PROVIDER_ID | "opencodezen")
            && self.base_url.trim_end_matches('/') == OPENCODE_ZEN_BASE_URL
    }
}
