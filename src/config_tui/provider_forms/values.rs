use super::*;
/// 【服务商配置】【请求头校验】解析自定义请求头 JSON 对象。
/// 参数: value 为字段文本；返回: 请求头映射，空文本返回空表
pub(super) fn normalize_extra_headers(
    value: &str,
) -> Result<std::collections::HashMap<String, String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    let obj = parsed.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            t(
                "Extra Headers JSON must be a JSON object of string values",
                "自定义请求头 JSON 必须是字符串键值对象"
            )
        )
    })?;
    let mut headers = std::collections::HashMap::new();
    for (key, val) in obj {
        let text = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !key.trim().is_empty() {
            headers.insert(key.clone(), text);
        }
    }
    Ok(headers)
}

/// 【服务商配置】【请求体校验】规范化自定义请求体。
/// 参数: value 为 JSON 文本；返回: 格式化对象，非法 JSON 返回错误
pub(super) fn normalize_extra_body(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    if !parsed.is_object() {
        bail!(
            "{}",
            t(
                "Custom Body JSON must be a JSON object",
                "自定义 Body JSON 必须是 JSON 对象"
            )
        );
    }
    Ok(serde_json::to_string_pretty(&parsed)?)
}

/// 规范化 provider Base URL。
///
/// 参数:
/// - `value`: 表单输入值
///
/// 返回:
/// - 去除末尾斜杠和 chat completions 后缀后的 URL
pub(super) fn normalize_base_url(value: &str) -> String {
    let mut url = value.trim().trim_end_matches('/').to_string();
    if url.ends_with("/chat/completions") {
        url.truncate(url.len() - "/chat/completions".len());
    }
    url
}
