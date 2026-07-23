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
    ensure_secret_sentinels_resolved(&submitted)?;
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

/// 校验浏览器提交内容中不存在未解析的敏感字段占位符。
///
/// 参数:
/// - `value`: 已完成旧配置合并的 JSON 数据
///
/// 返回:
/// - 所有占位符均已恢复时返回成功；标识变更导致无法匹配时返回错误
pub(crate) fn ensure_secret_sentinels_resolved(value: &Value) -> Result<()> {
    if contains_secret_sentinel(value) {
        anyhow::bail!(
            "secret value must be entered again after changing a provider or MCP server id"
        );
    }
    Ok(())
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
            // 1. 带稳定 id 的对象数组按 id 对齐，避免删除或排序后串用旧密钥
            let has_stable_ids = submitted
                .iter()
                .chain(current.iter())
                .any(|value| value.get("id").and_then(Value::as_str).is_some());
            let current_by_id = if has_stable_ids {
                current
                    .iter()
                    .filter_map(|value| {
                        value
                            .get("id")
                            .and_then(Value::as_str)
                            .map(|id| (id, value))
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            } else {
                std::collections::HashMap::new()
            };
            for (index, value) in submitted.iter_mut().enumerate() {
                let current_value = if has_stable_ids {
                    value
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(|id| current_by_id.get(id).copied())
                } else {
                    current.get(index)
                };
                if let Some(current_value) = current_value {
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

/// 递归判断 JSON 数据中是否仍包含敏感字段占位符。
///
/// 参数:
/// - `value`: 待检查 JSON 数据
///
/// 返回:
/// - 是否存在未解析的占位符
fn contains_secret_sentinel(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.values().any(contains_secret_sentinel),
        Value::Array(values) => values.iter().any(contains_secret_sentinel),
        Value::String(value) => value == SECRET_SENTINEL,
        _ => false,
    }
}

/// 判断配置键是否包含敏感凭据。
fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace('-', "_");
    key == "token"
        || key.ends_with("_token")
        || key.ends_with("_tokens")
        || key.ends_with("api_key")
        || key.ends_with("api_keys")
        || key.ends_with("secret")
        || key.ends_with("password")
        || key.ends_with("webhook_url")
        || key == "authorization"
        || key == "proxy_authorization"
        || key == "cookie"
        || key == "set_cookie"
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

    #[test]
    fn redacts_hyphenated_credential_headers() {
        let mut value = json!({ "headers": { "X-API-Key": "secret" } });

        redact_value(&mut value, None);

        assert_eq!(value["headers"]["X-API-Key"], SECRET_SENTINEL);
    }

    #[test]
    fn restores_provider_secret_by_id_after_reordering() {
        let current = json!({
            "providers": [
                { "id": "provider-a", "api_key": "secret-a" },
                { "id": "provider-b", "api_key": "secret-b" }
            ]
        });
        let mut submitted = json!({
            "providers": [
                { "id": "provider-b", "api_key": SECRET_SENTINEL },
                { "id": "provider-a", "api_key": SECRET_SENTINEL }
            ]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(submitted["providers"][0]["api_key"], "secret-b");
        assert_eq!(submitted["providers"][1]["api_key"], "secret-a");
    }

    #[test]
    fn restores_remaining_provider_secret_after_deletion() {
        let current = json!({
            "providers": [
                { "id": "provider-a", "api_key": "secret-a" },
                { "id": "provider-b", "api_key": "secret-b" }
            ]
        });
        let mut submitted = json!({
            "providers": [
                { "id": "provider-b", "api_key": SECRET_SENTINEL }
            ]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(submitted["providers"][0]["api_key"], "secret-b");
    }

    #[test]
    fn restores_mcp_header_by_server_id_after_reordering() {
        let current = json!({
            "servers": [
                { "id": "server-a", "headers": { "authorization": "secret-a" } },
                { "id": "server-b", "headers": { "authorization": "secret-b" } }
            ]
        });
        let mut submitted = json!({
            "servers": [
                { "id": "server-b", "headers": { "authorization": SECRET_SENTINEL } },
                { "id": "server-a", "headers": { "authorization": SECRET_SENTINEL } }
            ]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(
            submitted["servers"][0]["headers"]["authorization"],
            "secret-b"
        );
        assert_eq!(
            submitted["servers"][1]["headers"]["authorization"],
            "secret-a"
        );
    }

    #[test]
    fn does_not_restore_secret_for_new_id_from_old_array_position() {
        let current = json!({
            "servers": [
                { "id": "old", "headers": { "authorization": "old-secret" } }
            ]
        });
        let mut submitted = json!({
            "servers": [
                { "id": "new", "headers": { "authorization": SECRET_SENTINEL } }
            ]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(
            submitted["servers"][0]["headers"]["authorization"],
            SECRET_SENTINEL
        );
        assert!(ensure_secret_sentinels_resolved(&submitted).is_err());
    }
}
