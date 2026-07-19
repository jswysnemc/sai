use crate::config::{AgentProfile, AppConfig};
use anyhow::{bail, Result};
use serde::Serialize;

/// 输入区可编辑的 Agent 运行参数摘要。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct AgentRuntimeProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider_id: String,
    pub(crate) model: String,
    pub(crate) thinking_level: String,
}

impl From<&AgentProfile> for AgentRuntimeProfile {
    /// 将完整 Agent 档案转换为输入区所需的运行参数摘要。
    ///
    /// 参数:
    /// - `profile`: 完整 Agent 档案
    ///
    /// 返回:
    /// - 不包含提示词和能力列表的运行参数摘要
    fn from(profile: &AgentProfile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            provider_id: profile.provider_id.clone(),
            model: profile.model.clone(),
            thinking_level: profile.thinking_level.clone(),
        }
    }
}

/// 列出可由输入区快速配置的 Agent 运行参数。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 已解析内置覆盖和旧配置后的 Agent 摘要
pub(crate) fn list_profiles(config: &AppConfig) -> Vec<AgentRuntimeProfile> {
    config
        .resolved_agent_profiles()
        .iter()
        .map(AgentRuntimeProfile::from)
        .collect()
}

/// 更新单个 Agent 的模型与思考等级，并保留完整档案内容。
///
/// 参数:
/// - `config`: 待修改的应用配置
/// - `agent_id`: Agent 标识
/// - `provider_id`: 模型供应商标识，空值表示继承当前模型
/// - `model`: 模型标识，空值表示继承当前模型
/// - `thinking_level`: 思考等级
///
/// 返回:
/// - 更新后的运行参数摘要
pub(crate) fn update_profile(
    config: &mut AppConfig,
    agent_id: &str,
    provider_id: &str,
    model: &str,
    thinking_level: &str,
) -> Result<AgentRuntimeProfile> {
    let agent_id = agent_id.trim();
    let provider_id = provider_id.trim();
    let model = model.trim();
    let thinking_level = thinking_level.trim();
    if agent_id.is_empty() {
        bail!("agent id cannot be empty");
    }
    if provider_id.is_empty() != model.is_empty() {
        bail!("agent model and provider must be configured together");
    }
    if !provider_id.is_empty() {
        let provider = config
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| anyhow::anyhow!("agent provider not found: {provider_id}"))?;
        let configured =
            provider.models.iter().any(|item| item == model) || provider.default_model == model;
        if !configured {
            bail!("agent model is not enabled for provider {provider_id}: {model}");
        }
    }
    if !matches!(
        thinking_level,
        "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        bail!("agent thinking level is invalid: {thinking_level}");
    }

    // 1. 从已解析列表获取完整档案，避免首次覆盖内置 Agent 时丢失提示词和能力配置
    let mut profile = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == agent_id)
        .ok_or_else(|| anyhow::anyhow!("agent not found: {agent_id}"))?;
    // 2. 只修改输入区允许快速调整的运行参数
    profile.provider_id = provider_id.to_string();
    profile.model = model.to_string();
    profile.thinking_level = thinking_level.to_string();
    // 3. 已持久化档案原位替换，内置档案首次修改时追加完整解析结果
    if let Some(stored) = config
        .agents
        .iter_mut()
        .find(|stored| stored.id == agent_id)
    {
        *stored = profile.clone();
    } else {
        config.agents.push(profile.clone());
    }
    config.validate()?;
    Ok(AgentRuntimeProfile::from(&profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_builtin_profile_without_dropping_prompt_or_tools() {
        let mut config = AppConfig::default();
        let provider = config.providers[0].clone();

        let updated = update_profile(
            &mut config,
            "explore",
            &provider.id,
            &provider.default_model,
            "high",
        )
        .unwrap();

        assert_eq!(updated.thinking_level, "high");
        let stored = config
            .agents
            .iter()
            .find(|profile| profile.id == "explore")
            .unwrap();
        assert!(!stored.system_prompt.is_empty());
        assert!(!stored.enabled_tools.is_empty());
    }

    #[test]
    fn rejects_models_that_are_not_enabled_for_provider() {
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();

        let error = update_profile(
            &mut config,
            "general",
            &provider_id,
            "missing-model",
            "auto",
        )
        .unwrap_err();

        assert!(error.to_string().contains("not enabled"));
        assert!(config.agents.is_empty());
    }
}
