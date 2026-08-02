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

/// 读取指定供应商实际使用的 API Key。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider_id`: 供应商稳定标识
/// - `key_id`: 多密钥场景下指定要查看的密钥标识；缺省返回单值密钥
///
/// 返回:
/// - 解析直接配置、环境变量或独立密钥文件后的真实 API Key
pub(crate) fn load_provider_secret(
    paths: &SaiPaths,
    provider_id: &str,
    key_id: Option<&str>,
) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    let provider = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .with_context(|| format!("provider {provider_id} not found"))?;
    // 1. 指定了密钥标识时，按标识在多密钥列表中查找并解析
    if let Some(key_id) = key_id {
        let entry = provider
            .api_keys
            .iter()
            .find(|entry| entry.id == key_id)
            .with_context(|| format!("provider {provider_id} key {key_id} not found"))?;
        return resolve_key_value(&entry.api_key);
    }
    provider.resolved_api_key(paths)
}

/// 展开单个密钥值中的 `$env:` 引用。
///
/// 参数:
/// - `value`: 原始密钥文本
///
/// 返回:
/// - 解析后的密钥；空值返回错误
fn resolve_key_value(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("api key is empty");
    }
    if let Some(env_name) = trimmed.strip_prefix("$env:") {
        return std::env::var(env_name)
            .with_context(|| format!("environment variable {env_name} is not set"));
    }
    Ok(trimmed.to_string())
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
            restore_legacy_provider_key_into_multi_keys(submitted, current);
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

/// 将旧版单密钥配置迁移到前端统一编辑的多密钥列表。
///
/// 旧配置只有 `api_key`，前端编辑列表后会把原密钥放到 `key-1` 并提交脱敏占位符；
/// 当前配置没有多密钥数组时，按约定把旧字段的真实值回填到该位置。
///
/// 参数:
/// - `submitted`: 浏览器提交的对象
/// - `current`: 服务端保存的旧对象
///
/// 返回:
/// - 无；直接修改提交对象
fn restore_legacy_provider_key_into_multi_keys(
    submitted: &mut serde_json::Map<String, Value>,
    current: &serde_json::Map<String, Value>,
) {
    let has_current_multi_keys = current
        .get("api_keys")
        .and_then(Value::as_array)
        .is_some_and(|keys| !keys.is_empty());
    if has_current_multi_keys {
        return;
    }
    let Some(legacy_key) = current.get("api_key").and_then(Value::as_str) else {
        return;
    };
    if legacy_key.is_empty() {
        return;
    }
    let Some(Value::Array(keys)) = submitted.get_mut("api_keys") else {
        return;
    };
    for key in keys {
        let is_legacy_slot = key.get("id").and_then(Value::as_str) == Some("key-1");
        let keeps_legacy_secret =
            key.get("api_key").and_then(Value::as_str) == Some(SECRET_SENTINEL);
        if is_legacy_slot && keeps_legacy_secret {
            if let Some(object) = key.as_object_mut() {
                object.insert("api_key".to_string(), Value::String(legacy_key.to_string()));
            }
            break;
        }
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
    use std::path::Path;

    /// 创建仅供配置服务测试使用的临时路径集合。
    ///
    /// 参数:
    /// - `root`: 临时目录根路径
    ///
    /// 返回:
    /// - 指向临时目录的 Sai 路径集合
    fn test_paths(root: &Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/sai.ps1"),
        }
    }

    /// 真实密钥只允许通过按需读取入口返回，普通配置响应必须继续脱敏。
    #[test]
    fn reveals_provider_secret_without_changing_redacted_config() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let mut config = AppConfig::default();
        let provider = config.providers.first_mut().unwrap();
        provider.id = "provider-a".to_string();
        provider.api_key = Some("provider-secret".to_string());
        config.save(&paths).unwrap();

        let secret = load_provider_secret(&paths, "provider-a", None).unwrap();
        let redacted = load_redacted(&paths).unwrap();

        assert_eq!(secret, "provider-secret");
        assert_eq!(redacted["providers"][0]["api_key"], SECRET_SENTINEL);
    }

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

    /// 多密钥数组里的密钥字段必须脱敏，避免明文随配置响应泄漏到浏览器。
    #[test]
    fn redacts_multi_key_values_inside_provider() {
        let mut value = json!({
            "providers": [{
                "id": "p",
                "api_keys": [
                    { "id": "k1", "api_key": "secret-1", "label": "主号" },
                    { "id": "k2", "api_key": "$env:BACKUP_KEY", "label": "备用" }
                ]
            }]
        });

        redact_value(&mut value, None);

        assert_eq!(
            value["providers"][0]["api_keys"][0]["api_key"],
            SECRET_SENTINEL
        );
        // 环境变量引用保持可见，便于用户在界面上看到自己写的变量名
        assert_eq!(
            value["providers"][0]["api_keys"][1]["api_key"],
            "$env:BACKUP_KEY"
        );
        // 备注不是敏感字段，原样保留
        assert_eq!(value["providers"][0]["api_keys"][0]["label"], "主号");
    }

    /// 多密钥哨兵按稳定 id 回填，删除或重排后不串用密钥。
    #[test]
    fn restores_multi_key_sentinels_by_stable_id() {
        let current = json!({
            "providers": [{
                "id": "p",
                "api_keys": [
                    { "id": "k1", "api_key": "secret-1" },
                    { "id": "k2", "api_key": "secret-2" }
                ]
            }]
        });
        let mut submitted = json!({
            "providers": [{
                "id": "p",
                "api_keys": [
                    { "id": "k2", "api_key": SECRET_SENTINEL },
                    { "id": "k1", "api_key": SECRET_SENTINEL }
                ]
            }]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(
            submitted["providers"][0]["api_keys"][0]["api_key"],
            "secret-2"
        );
        assert_eq!(
            submitted["providers"][0]["api_keys"][1]["api_key"],
            "secret-1"
        );
    }

    /// 旧版单密钥编辑为多密钥列表时，原密钥必须继续保留。
    #[test]
    fn restores_legacy_single_key_when_migrating_to_multi_key_list() {
        let current = json!({
            "providers": [{
                "id": "p",
                "api_key": "legacy-secret"
            }]
        });
        let mut submitted = json!({
            "providers": [{
                "id": "p",
                "api_key": "",
                "api_keys": [
                    { "id": "key-1", "api_key": SECRET_SENTINEL, "label": "主号" },
                    { "id": "key-2", "api_key": "new-secret", "label": "备用" }
                ]
            }]
        });

        merge_secret_sentinels(&mut submitted, &current);

        assert_eq!(
            submitted["providers"][0]["api_keys"][0]["api_key"],
            "legacy-secret"
        );
        assert_eq!(
            submitted["providers"][0]["api_keys"][1]["api_key"],
            "new-secret"
        );
        assert!(ensure_secret_sentinels_resolved(&submitted).is_ok());
    }
}
