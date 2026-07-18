use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use serde_json::Value;

pub(crate) const SECRET_SENTINEL: &str = "__SAI_SECRET_UNCHANGED__";

/// 读取并脱敏 Sai 配置。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 脱敏后的 JSON 配置
pub(crate) fn load_redacted(paths: &SaiPaths) -> Result<Value> {
    let config = AppConfig::load_or_default(paths)?;
    let mut value = serde_json::to_value(config)?;
    // MCP 已独立到 mcp.jsonc，主配置 API 不再暴露该字段
    if let Some(object) = value.as_object_mut() {
        object.remove("mcp");
    }
    redact_value(&mut value, None);
    Ok(value)
}

/// 合并敏感字段保留标记并保存配置。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `submitted`: 浏览器提交配置
///
/// 返回:
/// - 保存后的脱敏配置
pub(crate) fn save(paths: &SaiPaths, mut submitted: Value) -> Result<Value> {
    // 主配置保存忽略 mcp；请走 /api/config/mcp
    if let Some(object) = submitted.as_object_mut() {
        object.remove("mcp");
    }
    let current = serde_json::to_value(AppConfig::load_or_default(paths)?)?;
    merge_secret_sentinels(&mut submitted, &current);
    let config: AppConfig =
        serde_json::from_value(submitted).context("invalid Sai configuration")?;
    config.validate()?;
    config.save(paths)?;
    load_redacted(paths)
}

/// 对外暴露的脱敏入口。
pub(crate) fn redact_json_value(value: &mut Value) {
    redact_value(value, None);
}

/// 对外暴露的敏感字段合并入口。
pub(crate) fn merge_secret_sentinels_json(submitted: &mut Value, current: &Value) {
    merge_secret_sentinels(submitted, current);
}

/// 递归隐藏配置中的敏感字符串。
fn redact_value(value: &mut Value, key: Option<&str>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                redact_value(value, Some(key));
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value, key);
            }
        }
        Value::String(text) if key.is_some_and(is_sensitive_key) => {
            if !text.trim().is_empty() && !text.trim_start().starts_with("$env:") {
                *text = SECRET_SENTINEL.to_string();
            }
        }
        _ => {}
    }
}

/// 使用当前配置替换浏览器传回的敏感字段保留标记。
fn merge_secret_sentinels(submitted: &mut Value, current: &Value) {
    match (submitted, current) {
        (Value::Object(submitted), Value::Object(current)) => {
            for (key, value) in submitted {
                if let Some(current_value) = current.get(key) {
                    merge_secret_sentinels(value, current_value);
                }
            }
        }
        (Value::Array(submitted), Value::Array(current)) => {
            for (index, value) in submitted.iter_mut().enumerate() {
                if let Some(current_value) = current.get(index) {
                    merge_secret_sentinels(value, current_value);
                }
            }
        }
        (Value::String(value), current) if value == SECRET_SENTINEL => {
            *value = current.as_str().unwrap_or_default().to_string();
        }
        _ => {}
    }
}

/// 判断配置键是否包含敏感凭据。
fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "token"
        || key.ends_with("_token")
        || key.ends_with("_tokens")
        || key.ends_with("api_key")
        || key.ends_with("api_keys")
        || key.ends_with("secret")
        || key.ends_with("password")
        || key.ends_with("webhook_url")
        || key == "authorization"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_and_restores_sensitive_values() {
        let current = json!({
            "providers": [{ "api_key": "secret", "base_url": "https://example.test" }],
            "gateways": { "qq": { "token": "gateway-secret" } }
        });
        let mut redacted = current.clone();
        redact_value(&mut redacted, None);
        assert_eq!(redacted["providers"][0]["api_key"], SECRET_SENTINEL);
        assert_eq!(redacted["gateways"]["qq"]["token"], SECRET_SENTINEL);
        merge_secret_sentinels(&mut redacted, &current);
        assert_eq!(redacted, current);
    }

    #[test]
    fn keeps_environment_references_visible() {
        let mut value = json!({ "api_key": "$env:OPENAI_API_KEY" });
        redact_value(&mut value, None);
        assert_eq!(value["api_key"], "$env:OPENAI_API_KEY");
    }

    #[test]
    fn redacts_plugin_key_arrays() {
        let mut value = json!({ "tinyfish_api_keys": ["first", "$env:TINYFISH_KEY"] });
        redact_value(&mut value, None);
        assert_eq!(value["tinyfish_api_keys"][0], SECRET_SENTINEL);
        assert_eq!(value["tinyfish_api_keys"][1], "$env:TINYFISH_KEY");
    }
}
