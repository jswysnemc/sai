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
    if !matches!(
        level.as_str(),
        "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max"
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
}
