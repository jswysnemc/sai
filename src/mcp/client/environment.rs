use std::collections::HashMap;

/// 展开配置中的 `$env:VAR` 引用。
///
/// 参数:
/// - `value`: 待展开的配置值
///
/// 返回:
/// - 环境变量值，非 `$env:` 前缀时返回原值
pub(super) fn expand_env_value(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(name) = trimmed.strip_prefix("$env:") {
        let name = name.trim();
        if name.is_empty() {
            return String::new();
        }
        return std::env::var(name).unwrap_or_default();
    }
    value.to_string()
}

/// 展开字符串键值表中的 `$env:` 引用。
///
/// 参数:
/// - `map`: 待展开的字符串键值表
///
/// 返回:
/// - 展开后的新键值表
pub(super) fn expand_env_map(map: &HashMap<String, String>) -> HashMap<String, String> {
    map.iter()
        .map(|(key, value)| (key.clone(), expand_env_value(value)))
        .collect()
}
