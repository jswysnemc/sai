use super::agent_override::apply_agent_override;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};

/// 读取配置并应用 Web 单轮模型覆盖。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider_id`: 可选供应商标识
/// - `model`: 可选模型标识
/// - `thinking_level`: 可选思考等级
///
/// 返回:
/// - 未指定覆盖时返回 `None`，否则返回临时配置
pub(crate) fn resolve_run_config(
    paths: &SaiPaths,
    agent_id: Option<&str>,
    provider_id: Option<&str>,
    model: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<Option<AppConfig>> {
    if agent_id.is_none() && provider_id.is_none() && model.is_none() && thinking_level.is_none() {
        return Ok(None);
    }
    let mut config = AppConfig::load_or_default(paths)?;
    config = apply_agent_override(config, agent_id)?;
    if config.agent.engine.is_external() {
        apply_acp_overrides(&mut config, provider_id, model, thinking_level)?;
        return Ok(Some(config));
    }
    match (provider_id, model) {
        (Some(provider_id), Some(model)) => {
            config = apply_model_override(config, provider_id, model)?;
        }
        (None, None) => {}
        _ => bail!("provider_id and model must be provided together"),
    }
    if let Some(level) = thinking_level {
        apply_thinking_override(&mut config, level)?;
    }
    Ok(Some(config))
}

/// 将 Web 单轮模型与思考选择映射到 ACP 标准配置类别。
///
/// 参数:
/// - `config`: 已应用 Agent 覆盖的运行配置
/// - `provider_id`: 前端 ACP 虚拟供应商标识
/// - `model`: 可选 ACP 模型值
/// - `thinking_level`: 可选 ACP 思考等级值
///
/// 返回:
/// - 覆盖是否有效
fn apply_acp_overrides(
    config: &mut AppConfig,
    provider_id: Option<&str>,
    model: Option<&str>,
    thinking_level: Option<&str>,
) -> Result<()> {
    match (provider_id, model) {
        (Some(provider_id), Some(model)) => {
            if provider_id != "__acp__" {
                bail!("external ACP runs require provider_id __acp__");
            }
            config.agent.acp.model = model.trim().to_string();
        }
        (None, None) => {}
        _ => bail!("provider_id and model must be provided together"),
    }
    if let Some(level) = thinking_level {
        let level = level.trim();
        if !level.is_empty() && !level.eq_ignore_ascii_case("auto") {
            config.agent.acp.thought_level = level.to_string();
        }
    }
    Ok(())
}

/// 对当前供应商应用单轮思考等级覆盖。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `level`: 思考等级
///
/// 返回:
/// - 覆盖是否成功
fn apply_thinking_override(config: &mut AppConfig, level: &str) -> Result<()> {
    let level = level.trim().to_ascii_lowercase();
    // TUI 的 auto 表示沿用 provider/agent 配置，不覆盖当前值。
    if level.is_empty() || level == "auto" {
        return Ok(());
    }
    if !matches!(
        level.as_str(),
        "none" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        bail!("unsupported thinking level: {level}");
    }
    let active_provider = config.active_provider.clone();
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == active_provider)
        .ok_or_else(|| anyhow::anyhow!("provider not found: {active_provider}"))?;
    provider.thinking_level = level;
    Ok(())
}

/// 对内存配置应用供应商和模型覆盖。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `provider_id`: 供应商标识
/// - `model`: 模型标识
///
/// 返回:
/// - 已应用覆盖的配置
fn apply_model_override(
    mut config: AppConfig,
    provider_id: &str,
    model: &str,
) -> Result<AppConfig> {
    let provider_id = provider_id.trim();
    let model = model.trim();
    if provider_id.is_empty() {
        bail!("provider_id cannot be empty");
    }
    config.set_active_provider_model(provider_id, model)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_provider_and_model_without_persisting() {
        let config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();
        let updated = apply_model_override(config, &provider_id, "test-model").unwrap();
        assert_eq!(updated.active_provider, provider_id);
        assert_eq!(updated.provider(None).unwrap().default_model, "test-model");
    }

    #[test]
    fn rejects_empty_provider_id() {
        let error = apply_model_override(AppConfig::default(), "", "test-model").unwrap_err();
        assert!(error.to_string().contains("provider_id cannot be empty"));
    }

    #[test]
    fn applies_thinking_level_to_active_provider() {
        let mut config = AppConfig::default();
        apply_thinking_override(&mut config, "xhigh").unwrap();
        assert_eq!(config.provider(None).unwrap().thinking_level, "xhigh");
    }

    #[test]
    fn auto_thinking_level_inherits_provider_config_like_tui() {
        let mut config = AppConfig::default();
        let active_provider = config.active_provider.clone();
        config
            .providers
            .iter_mut()
            .find(|provider| provider.id == active_provider)
            .unwrap()
            .thinking_level = "high".to_string();

        apply_thinking_override(&mut config, "auto").unwrap();

        assert_eq!(config.provider(None).unwrap().thinking_level, "high");
    }

    /// 外部内核的单轮选择必须写入 ACP 配置，而不是内置供应商。
    #[test]
    fn applies_external_model_and_thinking_to_acp() {
        let mut config = AppConfig::default();
        config.agent.engine = crate::config::AgentEngineKind::ClaudeCode;
        apply_acp_overrides(
            &mut config,
            Some("__acp__"),
            Some("claude-sonnet"),
            Some("high"),
        )
        .unwrap();

        assert_eq!(config.agent.acp.model, "claude-sonnet");
        assert_eq!(config.agent.acp.thought_level, "high");
    }

    #[test]
    fn auto_thinking_level_inherits_acp_config() {
        let mut config = AppConfig::default();
        config.agent.engine = crate::config::AgentEngineKind::ClaudeCode;
        config.agent.acp.thought_level = "high".to_string();

        apply_acp_overrides(&mut config, None, None, Some("auto")).unwrap();

        assert_eq!(config.agent.acp.thought_level, "high");
    }
}
