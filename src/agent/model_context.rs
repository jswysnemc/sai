use crate::config::AppConfig;
use anyhow::Result;

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
