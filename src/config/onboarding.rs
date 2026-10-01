//! 首次供应商配置：TUI 与 Web 共用字段校验、默认模型设置和完成状态。

use super::{AppConfig, ProviderConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// 首次配置表单；空 provider_id 表示创建自定义供应商，空密钥保留已有凭据。
#[derive(Clone, Deserialize)]
pub(crate) struct ProviderSetupInput {
    pub provider_id: Option<String>,
    pub display_name: String,
    pub base_url: String,
    pub protocol: String,
    pub api_key: Option<String>,
    pub model: String,
}

/// 【首次配置】【保存供应商】校验用户选择并持久化供应商、默认模型与两端共用的完成状态。
/// @param paths 为应用路径；input 为用户确认后的配置表单
/// @returns 保存后的完整配置；失败时不写入草稿，重复提交返回已完成配置
pub(crate) fn complete_provider_setup(
    paths: &SaiPaths,
    input: ProviderSetupInput,
) -> Result<AppConfig> {
    let mut config = AppConfig::load_or_default(paths)?;
    if config.provider_setup_complete {
        return Ok(config);
    }
    // 1. 【首次配置】【供应商选择】复用模板或已有配置，保留高级参数和多密钥设置
    let mut provider = match input.provider_id.as_deref() {
        Some(id) => config
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned()
            .with_context(|| t("Provider no longer exists", "供应商已不存在"))?,
        None => {
            let mut provider = ProviderConfig::new_openai_compatible();
            provider.id = custom_provider_id(&config);
            provider.api_key = None;
            provider
        }
    };
    provider.display_name = input.display_name.trim().to_string();
    provider.base_url = input.base_url.trim().trim_end_matches('/').to_string();
    if let Some(base) = provider.base_url.strip_suffix("/chat/completions") {
        provider.base_url = base.to_string();
    }
    provider.protocol = input.protocol.trim().to_string();
    provider.default_model = input.model.trim().to_string();
    provider.enabled = true;
    if provider.display_name.is_empty() || provider.default_model.is_empty() {
        bail!(
            "{}",
            t(
                "Enter a provider name and default model",
                "请填写供应商名称和默认模型"
            )
        );
    }
    let valid_url = reqwest::Url::parse(&provider.base_url)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
    if !valid_url {
        bail!(
            "{}",
            t(
                "Enter a valid HTTP(S) API address",
                "请填写有效的 HTTP(S) API 地址"
            )
        );
    }
    if !matches!(
        provider.protocol.as_str(),
        "auto" | "openai-chat" | "openai-responses" | "anthropic"
    ) {
        bail!(
            "{}",
            t("Select a supported API protocol", "请选择支持的接口协议")
        );
    }
    // 2. 【首次配置】【凭据校验】解析环境变量和独立密钥文件，不发起收费请求，也不记录密钥
    if let Some(key) = input.api_key.filter(|key| !key.trim().is_empty()) {
        provider.api_key = Some(key.trim().to_string());
        provider.api_keys.clear();
        provider.api_key_selected = None;
        provider.api_key_balance = false;
    }
    if provider.resolved_api_key(paths)?.trim().is_empty() {
        bail!("{}", t("The API key is empty", "API 密钥不能为空"));
    }
    if !provider.models.contains(&provider.default_model) {
        provider.models.push(provider.default_model.clone());
    }
    // 3. 【首次配置】【默认模型】主对话和 Web 新会话都使用用户刚确认的供应商与模型
    config.active_provider = provider.id.clone();
    config.session.new_session_provider_id = provider.id.clone();
    config.session.new_session_model = provider.default_model.clone();
    if let Some(existing) = config
        .providers
        .iter_mut()
        .find(|item| item.id == provider.id)
    {
        *existing = provider;
    } else {
        config.providers.push(provider);
    }
    config.provider_setup_complete = true;
    config.validate()?;
    config.save(paths)?;
    Ok(config)
}

/// 【首次配置】【自定义标识】生成未被已有供应商占用的稳定标识。
/// @param config 为当前配置
/// @returns 可用供应商标识
fn custom_provider_id(config: &AppConfig) -> String {
    let mut id = "custom".to_string();
    let mut suffix = 2;
    while config.providers.iter().any(|provider| provider.id == id) {
        id = format!("custom-{suffix}");
        suffix += 1;
    }
    id
}
