use super::{AppConfig, ModelEndpointConfig, ModelEndpointKind};
use serde::Deserialize;
#[cfg(test)]
use serde_json::Value;

/// 旧版 `jev_routing` 段的全部字段；缺省字段保持新配置中的值。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyJevRouting {
    enabled: Option<bool>,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    threshold: Option<f64>,
    max_tools: Option<usize>,
    max_skills: Option<usize>,
    timeout_seconds: Option<u64>,
    context_chars: Option<usize>,
}

/// 旧版未修改时的地址、密钥与模型。
const LEGACY_DEFAULT_BASE_URL: &str = "https://api.typesafe.ai/v1";
const LEGACY_DEFAULT_API_KEYS: [&str; 2] = ["", "$env:TYPESAFE_API_KEY"];

impl AppConfig {
    /// 【Jev配置】【旧版迁移】把旧版 `jev_routing` 段并入 `jev.routing`。
    ///
    /// 旧版自带地址与密钥；与官方默认值不同时迁移为一条 JEV 接入，
    /// 已有 JEV 接入时不再新建，避免覆盖用户在设置页维护的接入。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；旧段无法解析时忽略
    pub(crate) fn migrate_legacy_jev_routing(&mut self) {
        let Some(raw) = self.legacy_jev_routing.take() else {
            return;
        };
        let Ok(legacy) = serde_json::from_value::<LegacyJevRouting>(raw) else {
            return;
        };
        // 1. 决策参数逐项覆盖
        let routing = &mut self.jev.routing;
        if let Some(value) = legacy.enabled {
            routing.enabled = value;
        }
        if let Some(value) = legacy.threshold {
            routing.threshold = value;
        }
        if let Some(value) = legacy.max_tools {
            routing.max_tools = value;
        }
        if let Some(value) = legacy.max_skills {
            routing.max_skills = value;
        }
        if let Some(value) = legacy.timeout_seconds {
            routing.timeout_seconds = value;
        }
        if let Some(value) = legacy.context_chars {
            routing.context_chars = value;
        }
        // 2. 自定义地址、密钥或模型迁移为 JEV 接入
        let base_url = legacy.base_url.unwrap_or_default();
        let api_key = legacy.api_key.unwrap_or_default();
        let model = legacy.model.unwrap_or_default();
        let customized = (!base_url.trim().is_empty()
            && base_url.trim().trim_end_matches('/') != LEGACY_DEFAULT_BASE_URL)
            || !LEGACY_DEFAULT_API_KEYS.contains(&api_key.trim())
            || !matches!(model.trim(), "" | super::JEV_DEFAULT_MODEL);
        let has_endpoint = self
            .model_endpoints
            .iter()
            .any(|item| item.kind == ModelEndpointKind::Jev);
        if customized && !has_endpoint {
            let base = if base_url.trim().is_empty() {
                LEGACY_DEFAULT_BASE_URL.to_string()
            } else {
                base_url.trim().trim_end_matches('/').to_string()
            };
            self.model_endpoints.push(ModelEndpointConfig {
                id: "jev-1".to_string(),
                kind: ModelEndpointKind::Jev,
                name: "TypeSafe Jev".to_string(),
                endpoint: format!("{base}/systemone"),
                protocol: "auto".to_string(),
                api_key,
                api_keys: Vec::new(),
                api_key_selected: None,
                api_key_balance: false,
                models: Vec::new(),
                model,
            });
        }
    }
}

/// 构造只含旧版段的配置，供测试使用。
#[cfg(test)]
fn with_legacy(value: Value) -> AppConfig {
    let mut config = AppConfig::default();
    config.legacy_jev_routing = Some(value);
    config.migrate_legacy_jev_routing();
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_legacy_section_only_moves_routing_fields() {
        let config = with_legacy(json!({"enabled": true, "max_tools": 2, "api_key": "$env:TYPESAFE_API_KEY"}));
        assert!(config.jev.routing.enabled);
        assert_eq!(config.jev.routing.max_tools, 2);
        assert!(config.model_endpoints.is_empty());
        assert!(config.legacy_jev_routing.is_none());
    }

    #[test]
    fn custom_connection_becomes_endpoint() {
        let config = with_legacy(json!({"base_url": "http://localhost:9087/v1/", "api_key": "$env:MY_KEY"}));
        let endpoint = &config.model_endpoints[0];
        assert_eq!(endpoint.kind, ModelEndpointKind::Jev);
        assert_eq!(endpoint.endpoint, "http://localhost:9087/v1/systemone");
        assert_eq!(endpoint.api_key, "$env:MY_KEY");
    }

    #[test]
    fn saved_config_drops_legacy_section() {
        let config = with_legacy(json!({"enabled": true}));
        let value = serde_json::to_value(&config).unwrap();
        assert!(value.get("jev_routing").is_none());
        assert_eq!(value["jev"]["routing"]["enabled"], true);
    }
}
