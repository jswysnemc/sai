use crate::config::AppConfig;
use anyhow::Result;

/// 返回当前配置实际使用的模型标识。
///
/// 外部内核取 ACP 配置的模型，内置内核取活动供应商的默认模型；
/// 两者与 Web 单轮覆盖后的取值一致，用于按轮持久化模型记录。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 非空模型标识；未配置模型时为 None
pub(super) fn current_model_id(config: &AppConfig) -> Option<String> {
    let model = if config.agent.engine.is_external() {
        config.agent.acp.model.trim().to_string()
    } else {
        config
            .provider(None)
            .ok()?
            .default_model
            .trim()
            .to_string()
    };
    (!model.is_empty()).then_some(model)
}

/// 构造当前配置的 provider/model 标签。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 当前 provider/model 标签
pub(super) fn selected_model_label(config: &AppConfig) -> Result<Option<String>> {
    let provider = config.provider(None)?;
    let model = provider.default_model.trim();
    if model.is_empty() {
        return Ok(None);
    }
    let provider_name = provider.display_name.trim();
    let provider_label = if provider_name.is_empty() {
        provider.id.trim()
    } else {
        provider_name
    };
    if provider_label.is_empty() {
        Ok(Some(model.to_string()))
    } else {
        Ok(Some(format!("{provider_label}/{model}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_model_label_uses_provider_name_and_model() {
        let mut config = AppConfig::default();
        let active_provider = config.active_provider.clone();
        let provider = config
            .providers
            .iter_mut()
            .find(|provider| provider.id == active_provider)
            .unwrap();
        provider.display_name = "Provider".to_string();
        provider.default_model = "model-x".to_string();

        let label = selected_model_label(&config).unwrap();

        assert_eq!(label, Some("Provider/model-x".to_string()));
    }
}
