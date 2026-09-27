use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// 未配置密钥时回退读取的环境变量。
const DEFAULT_API_KEY_ENV: &str = "TYPESAFE_API_KEY";

/// 基于 TypeSafe Jev 的工具与 skills 暴露决策配置。
///
/// 开启后非基础工具一律延迟暴露：每轮请求前由 Jev 预选，
/// 模型也可通过 `request_capability` 向 Jev 追加申请。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JevRoutingConfig {
    /// 是否开启 Jev 暴露决策
    pub enabled: bool,
    /// TypeSafe API 根地址
    pub base_url: String,
    /// API 密钥；支持 `$env:NAME` 引用，留空时读取 `TYPESAFE_API_KEY`
    pub api_key: String,
    /// Jev 模型名
    pub model: String,
    /// 候选被选中所需的最低 Noul 概率
    pub threshold: f64,
    /// 单次决策最多暴露的工具数
    pub max_tools: usize,
    /// 单次决策最多暴露的 skill 数
    pub max_skills: usize,
    /// 单次 Jev 请求超时秒数
    pub timeout_seconds: u64,
    /// 作为判断依据的近期对话最大字符数
    pub context_chars: usize,
}

impl Default for JevRoutingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://api.typesafe.ai/v1".to_string(),
            api_key: format!("$env:{DEFAULT_API_KEY_ENV}"),
            model: "jev-latest".to_string(),
            threshold: 0.5,
            max_tools: 6,
            max_skills: 3,
            timeout_seconds: 20,
            context_chars: 2_000,
        }
    }
}

impl JevRoutingConfig {
    /// 解析实际使用的 API 密钥。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 展开 `$env:` 后的非空密钥；缺失时报错
    pub fn resolved_api_key(&self) -> Result<String> {
        let raw = self.api_key.trim();
        // 1. 未配置时回退默认环境变量
        let env_name = if raw.is_empty() {
            Some(DEFAULT_API_KEY_ENV)
        } else {
            raw.strip_prefix("$env:")
        };
        // 2. 环境变量引用与明文密钥分别处理
        let key = match env_name {
            Some(name) => std::env::var(name)
                .with_context(|| format!("environment variable {name} is not set"))?,
            None => raw.to_string(),
        };
        if key.trim().is_empty() {
            bail!("jev_routing api key is empty");
        }
        Ok(key.trim().to_string())
    }

    /// 返回 systemone 评估端点地址。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 形如 `https://api.typesafe.ai/v1/systemone` 的地址
    pub fn endpoint(&self) -> String {
        format!("{}/systemone", self.base_url.trim().trim_end_matches('/'))
    }
}

impl super::AppConfig {
    /// 判断 Jev 暴露决策在当前会话是否生效。
    ///
    /// DeepSeek Anchored Standard 自带工具目录控制，两者同时开启时以锚定模式为准。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 开关打开且未启用锚定模式时返回 true
    pub fn jev_routing_active(&self) -> bool {
        self.jev_routing.enabled && !self.active_deepseek_anchor_enabled().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_joins_base_url_without_duplicate_slash() {
        let mut config = JevRoutingConfig::default();
        config.base_url = "https://api.typesafe.ai/v1/".to_string();
        assert_eq!(config.endpoint(), "https://api.typesafe.ai/v1/systemone");
    }

    #[test]
    fn literal_api_key_is_used_verbatim() {
        let mut config = JevRoutingConfig::default();
        config.api_key = " literal-key ".to_string();
        assert_eq!(config.resolved_api_key().unwrap(), "literal-key");
    }

    #[test]
    fn missing_env_reference_reports_variable_name() {
        let mut config = JevRoutingConfig::default();
        config.api_key = "$env:SAI_JEV_TEST_MISSING_KEY".to_string();
        let error = config.resolved_api_key().unwrap_err().to_string();
        assert!(error.contains("SAI_JEV_TEST_MISSING_KEY"));
    }

    #[test]
    fn partial_json_keeps_defaults() {
        let config: JevRoutingConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(config.enabled);
        assert_eq!(config.model, "jev-latest");
        assert_eq!(config.max_tools, 6);
    }
}
