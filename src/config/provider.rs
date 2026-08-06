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
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: vec![OPENCODE_DEFAULT_CHAT_MODEL.to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: OPENCODE_DEFAULT_CHAT_MODEL.to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            preserve_thinking: false,
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
            claude_1m_context: default_claude_1m_context(),
        }
    }

    pub fn default_openai() -> Self {
        Self {
            id: "openai".to_string(),
            display_name: "OpenAI-compatible".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            protocol: default_provider_protocol(),
            api_key: Some("$env:OPENAI_API_KEY".to_string()),
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: vec!["gpt-4o-mini".to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: "gpt-4o-mini".to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            preserve_thinking: false,
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
            claude_1m_context: default_claude_1m_context(),
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
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: vec!["claude-sonnet-4-5".to_string()],
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: "claude-sonnet-4-5".to_string(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            preserve_thinking: false,
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
            claude_1m_context: default_claude_1m_context(),
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
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: Vec::new(),
            model_context_chars: HashMap::new(),
            model_metadata: HashMap::new(),
            default_model: String::new(),
            timeout_seconds: default_timeout(),
            temperature: default_temperature(),
            anthropic_max_tokens: default_anthropic_max_tokens(),
            thinking_level: default_thinking_level(),
            thinking_format: default_thinking_format(),
            preserve_thinking: false,
            extra_body: String::new(),
            extra_headers: HashMap::new(),
            user_agent: String::new(),
            client_style: default_client_style(),
            claude_1m_context: default_claude_1m_context(),
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
    /// - 自定义 UA；未配置时 Codex/Claude 模式返回对应 CLI UA，否则返回 sai 默认 UA
    pub fn effective_user_agent(&self) -> String {
        let custom = self.user_agent.trim();
        if !custom.is_empty() {
            return custom.to_string();
        }
        let style = self.client_style.trim().to_ascii_lowercase();
        if style == "codex" {
            return super::defaults::CODEX_CLI_USER_AGENT.to_string();
        }
        if matches!(style.as_str(), "claude" | "claude-code" | "claude_code") {
            return super::defaults::CLAUDE_CLI_USER_AGENT.to_string();
        }
        super::defaults::DEFAULT_HTTP_USER_AGENT.to_string()
    }

    pub fn resolved_api_key(&self, paths: &SaiPaths) -> Result<String> {
        // 1. 多密钥列表优先：负载均衡时取首个作为默认，否则取选中项
        if !self.api_keys.is_empty() {
            if let Some(key) = self.chosen_multi_key()? {
                return Ok(key);
            }
        }

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

    /// 返回多密钥列表解析后的候选集，供负载均衡轮询。
    ///
    /// 仅当 `api_keys` 非空时返回非空列表；否则返回空，
    /// 调用方据此回退到单值 `resolved_api_key`。
    ///
    /// 返回:
    /// - 已展开 `$env:` 且非空的密钥列表
    pub fn resolved_api_keys(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        for entry in &self.api_keys {
            if let Some(key) = resolve_key_value(&entry.api_key)? {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    /// 取多密钥列表里当前应使用的那一个（用于默认解析与探测）。
    ///
    /// 负载均衡开启时返回首个密钥作为默认；关闭时优先用
    /// `api_key_selected` 命中的条目，否则取首个。真正的逐请求轮询由客户端完成。
    ///
    /// 返回:
    /// - 选中的密钥；列表为空时返回 None
    fn chosen_multi_key(&self) -> Result<Option<String>> {
        if self.api_keys.is_empty() {
            return Ok(None);
        }
        let chosen = if self.api_key_balance {
            self.api_keys.first()
        } else {
            self.api_keys
                .iter()
                .find(|entry| {
                    self.api_key_selected
                        .as_deref()
                        .is_some_and(|selected| selected == entry.id)
                })
                .or_else(|| self.api_keys.first())
        };
        match chosen {
            Some(entry) => resolve_key_value(&entry.api_key),
            None => Ok(None),
        }
    }

    pub fn is_opencode_zen(&self) -> bool {
        matches!(self.id.as_str(), OPENCODE_PROVIDER_ID | "opencodezen")
            && self.base_url.trim_end_matches('/') == OPENCODE_ZEN_BASE_URL
    }
}

/// 展开单个密钥值中的 `$env:` 引用。
///
/// 参数:
/// - `value`: 原始密钥文本
///
/// 返回:
/// - 解析后的密钥；空值返回 None，环境变量缺失时报错
fn resolve_key_value(value: &str) -> Result<Option<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Some(env_name) = trimmed.strip_prefix("$env:") {
        let resolved = std::env::var(env_name)
            .with_context(|| format!("environment variable {env_name} is not set"))?;
        return Ok((!resolved.is_empty()).then_some(resolved));
    }
    Ok(Some(trimmed.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造带多密钥的测试供应商。
    fn provider_with_keys(
        balance: bool,
        selected: Option<&str>,
        keys: &[(&str, &str)],
    ) -> ProviderConfig {
        let mut provider = ProviderConfig::default_openai();
        provider.api_keys = keys
            .iter()
            .map(|(id, value)| crate::config::ProviderApiKey {
                id: (*id).to_string(),
                api_key: (*value).to_string(),
                label: String::new(),
            })
            .collect();
        provider.api_key_balance = balance;
        provider.api_key_selected = selected.map(str::to_string);
        provider
    }

    /// 关闭负载均衡时取选中项。
    #[test]
    fn selected_key_wins_when_balance_is_off() {
        let provider = provider_with_keys(false, Some("b"), &[("a", "key-a"), ("b", "key-b")]);

        assert_eq!(provider.resolved_api_key(&test_paths()).unwrap(), "key-b");
    }

    /// 关闭负载均衡且无选中项时回落到首个。
    #[test]
    fn falls_back_to_first_when_no_selection() {
        let provider = provider_with_keys(false, None, &[("a", "key-a"), ("b", "key-b")]);

        assert_eq!(provider.resolved_api_key(&test_paths()).unwrap(), "key-a");
    }

    /// 候选集按列表顺序展开。
    #[test]
    fn resolved_keys_lists_every_entry() {
        let provider = provider_with_keys(true, None, &[("a", "key-a"), ("b", "key-b")]);

        assert_eq!(
            provider.resolved_api_keys().unwrap(),
            vec!["key-a", "key-b"]
        );
    }

    /// 多密钥为空时回落到单值字段。
    #[test]
    fn empty_multi_keys_falls_back_to_single_key() {
        let mut provider = ProviderConfig::default_openai();
        provider.api_key = Some("plain-key".to_string());

        assert_eq!(
            provider.resolved_api_key(&test_paths()).unwrap(),
            "plain-key"
        );
    }

    /// 仅供测试使用的空路径集合。
    fn test_paths() -> SaiPaths {
        SaiPaths {
            config_dir: std::path::PathBuf::new(),
            config_file: std::path::PathBuf::new(),
            secrets_file: std::path::PathBuf::new(),
            skills_dir: std::path::PathBuf::new(),
            data_dir: std::path::PathBuf::new(),
            cache_dir: std::path::PathBuf::new(),
            state_dir: std::path::PathBuf::new(),
            pictures_dir: std::path::PathBuf::new(),
            fish_hook_file: std::path::PathBuf::new(),
            bash_hook_file: std::path::PathBuf::new(),
            zsh_hook_file: std::path::PathBuf::new(),
            powershell_hook_file: std::path::PathBuf::new(),
        }
    }
}
