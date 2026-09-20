use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 独立模型接入类型；与普通对话供应商分开存储。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelEndpointKind {
    ImageGeneration,
    Jev,
}

/// 专用模型连接配置，仅保存接入信息，不绑定执行功能。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEndpointConfig {
    pub id: String,
    pub kind: ModelEndpointKind,
    pub name: String,
    /// 完整请求地址，包括端口及请求路径，不自动拼接普通聊天端点
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

impl ModelEndpointConfig {
    /// 解析专用端点的 API Key，支持配置文件中的环境变量引用。
    ///
    /// 返回:
    /// - 实际请求密钥；环境变量不存在时返回错误
    pub fn resolved_api_key(&self) -> Result<String> {
        let value = self.api_key.trim();
        if let Some(name) = value.strip_prefix("$env:") {
            let name = name.trim();
            if name.is_empty() {
                bail!("model endpoint API key environment variable is empty");
            }
            return std::env::var(name)
                .with_context(|| format!("environment variable {name} is not set"));
        }
        Ok(self.api_key.clone())
    }
}

/// 【模型接入】【配置校验】检查稳定标识和独立请求地址，不发起网络请求。
/// 参数: endpoints 为专用模型连接列表
/// 返回: 校验结果；错误不包含地址中的凭据或密钥
pub(super) fn validate(endpoints: &[ModelEndpointConfig]) -> Result<()> {
    let mut ids = HashSet::new();
    for item in endpoints {
        if item.id.trim().is_empty() || !ids.insert(&item.id) {
            bail!("model_endpoints ids must be non-empty and unique");
        }
        if item.name.trim().is_empty() {
            bail!("model_endpoints name cannot be empty");
        }
        let url = reqwest::Url::parse(&item.endpoint).map_err(|_| {
            anyhow::anyhow!("model_endpoints endpoint must be an absolute HTTP(S) URL")
        })?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || item.endpoint.chars().any(char::is_control)
        {
            bail!("model_endpoints endpoint must use HTTP(S) without credentials, fragments or control characters");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    /// 【模型接入】【旧配置】旧文档没有独立模型字段时继续使用原供应商；无参数，无返回值
    #[test]
    fn old_config_defaults_to_no_specialized_endpoints() {
        let mut raw = serde_json::to_value(AppConfig::default()).unwrap();
        raw.as_object_mut().unwrap().remove("model_endpoints");
        let config: AppConfig = serde_json::from_value(raw).unwrap();
        assert!(config.model_endpoints.is_empty());
        config.validate().unwrap();
    }

    /// 【模型接入】【地址验证】保留不同端口和完整路径，拒绝无效地址；无参数，无返回值
    #[test]
    fn validates_independent_endpoints_without_rewriting_them() {
        let mut endpoint = ModelEndpointConfig {
            id: "jev-local".into(),
            kind: ModelEndpointKind::Jev,
            name: "JEV".into(),
            endpoint: "http://localhost:9087/custom/decisions".into(),
            api_key: "key".into(),
            model: "jev-latest".into(),
        };
        validate(&[endpoint.clone()]).unwrap();
        assert!(validate(&[endpoint.clone(), endpoint.clone()]).is_err());
        for url in [
            "file:///tmp/model",
            "localhost:8000",
            "https://user:secret@example.com/api",
            "https://example.com/#fragment",
        ] {
            endpoint.endpoint = url.into();
            assert!(validate(&[endpoint.clone()]).is_err());
        }
    }
}
