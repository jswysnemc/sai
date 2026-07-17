use crate::config::ProviderConfig;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThinkingProtocol {
    OpenAiChat,
    OpenAiResponses,
    Anthropic,
}

/// 向请求体应用 provider 的思考参数和自定义 Body。
///
/// 参数:
/// - `body`: 待发送的 JSON 请求体
/// - `provider`: 当前 provider 配置
/// - `protocol`: 当前请求协议
///
/// 返回:
/// - 应用配置后的请求体
pub(crate) fn apply_provider_body_options(
    mut body: Value,
    provider: &ProviderConfig,
    protocol: ThinkingProtocol,
) -> Result<Value> {
    apply_thinking_options(&mut body, provider, protocol);
    apply_extra_body(&mut body, provider)?;
    Ok(body)
}

/// 向请求体应用思考参数。
///
/// 参数:
/// - `body`: 待修改的 JSON 请求体
/// - `provider`: 当前 provider 配置
/// - `protocol`: 当前请求协议
///
/// 返回:
/// - 无
fn apply_thinking_options(body: &mut Value, provider: &ProviderConfig, protocol: ThinkingProtocol) {
    let level = normalized_level(&provider.thinking_level);
    if level == "auto" {
        return;
    }
    match effective_format(provider, protocol) {
        "disabled" => {}
        "string" => {
            body["thinking"] = json!(level);
        }
        "object" => apply_generic_thinking_object(body, level),
        "deepseek-thinking" => apply_deepseek_thinking(body, level),
        "openai-chat-reasoning-effort" => {
            if level != "none" {
                body["reasoning_effort"] = json!(reasoning_effort(level));
            }
        }
        "reasoning" => {
            body["reasoning"] = json!({ "effort": reasoning_effort(level) });
        }
        "anthropic-thinking" => apply_anthropic_thinking(body, level),
        _ => {
            body["thinking"] = json!(level);
        }
    }
}

/// 计算当前实际使用的思考参数格式。
///
/// 参数:
/// - `provider`: 当前 provider 配置
/// - `protocol`: 当前请求协议
///
/// 返回:
/// - 思考参数格式标识
fn effective_format(provider: &ProviderConfig, protocol: ThinkingProtocol) -> &'static str {
    let configured = provider.thinking_format.trim();
    if is_deepseek_provider(provider) && configured != "disabled" {
        return "deepseek-thinking";
    }
    if !configured.is_empty() && configured != "auto" {
        return match configured {
            "string" => "string",
            "object" => "object",
            "deepseek-thinking" => "deepseek-thinking",
            "openai-chat-reasoning-effort" => "openai-chat-reasoning-effort",
            "reasoning" => "reasoning",
            "anthropic-thinking" => "anthropic-thinking",
            "disabled" => "disabled",
            _ => "string",
        };
    }
    match protocol {
        ThinkingProtocol::OpenAiResponses => "reasoning",
        ThinkingProtocol::Anthropic => "anthropic-thinking",
        ThinkingProtocol::OpenAiChat => "string",
    }
}

/// 判断 provider 是否为 DeepSeek 兼容供应商。
///
/// 参数:
/// - `provider`: 当前 provider 配置
///
/// 返回:
/// - 是否匹配 DeepSeek
fn is_deepseek_provider(provider: &ProviderConfig) -> bool {
    let id = provider.id.to_ascii_lowercase();
    let base_url = provider.base_url.to_ascii_lowercase();
    let model = provider.default_model.to_ascii_lowercase();
    id.contains("deepseek") || base_url.contains("deepseek") || model.contains("deepseek")
}

/// 规范化思考等级。
///
/// 参数:
/// - `level`: 原始等级
///
/// 返回:
/// - 可识别的等级
fn normalized_level(level: &str) -> &str {
    match level.trim() {
        "" => "auto",
        "auto" => "auto",
        "none" | "off" | "disabled" => "none",
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        "xhigh" => "xhigh",
        "max" => "max",
        _ => "auto",
    }
}

/// 映射为 reasoning effort。
///
/// 参数:
/// - `level`: 思考等级
///
/// 返回:
/// - reasoning effort 等级
fn reasoning_effort(level: &str) -> &'static str {
    match level {
        "none" => "minimal",
        "low" => "low",
        "medium" => "medium",
        "xhigh" | "max" => "xhigh",
        _ => "high",
    }
}

/// 映射为 DeepSeek reasoning_effort。
///
/// 参数:
/// - `level`: 思考等级
///
/// 返回:
/// - DeepSeek 支持的 effort 等级
fn deepseek_effort(level: &str) -> &'static str {
    match level {
        "max" | "xhigh" => "max",
        _ => "high",
    }
}

/// 映射为思考 token 预算。
///
/// 参数:
/// - `level`: 思考等级
///
/// 返回:
/// - token 预算
fn thinking_budget(level: &str) -> u64 {
    match level {
        "max" => 8192,
        "xhigh" => 6144,
        "high" => 4096,
        "low" => 1024,
        _ => 2048,
    }
}

/// 写入通用对象格式 thinking。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_generic_thinking_object(body: &mut Value, level: &str) {
    if level == "none" {
        body["thinking"] = json!({ "enabled": false });
        return;
    }
    body["thinking"] = json!({
        "enabled": true,
        "level": level,
        "budget_tokens": thinking_budget(level),
    });
}

/// 写入 DeepSeek OpenAI 兼容思考参数。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_deepseek_thinking(body: &mut Value, level: &str) {
    if level == "none" {
        body["thinking"] = json!({ "type": "disabled" });
        if let Some(object) = body.as_object_mut() {
            object.remove("reasoning_effort");
        }
        return;
    }
    body["thinking"] = json!({ "type": "enabled" });
    body["reasoning_effort"] = json!(deepseek_effort(level));
}

/// 写入 Anthropic 扩展思考参数。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_anthropic_thinking(body: &mut Value, level: &str) {
    if level == "none" {
        return;
    }
    let budget = thinking_budget(level);
    let current_max = body
        .get("max_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(4096);
    body["max_tokens"] = json!(current_max.max(budget + 1024));
    body["thinking"] = json!({
        "type": "enabled",
        "budget_tokens": budget,
    });
}

/// 合并 provider 自定义请求体字段。
///
/// 参数:
/// - `body`: 待修改的 JSON 请求体
/// - `provider`: 当前 provider 配置
///
/// 返回:
/// - 自定义 JSON 非法时返回错误
fn apply_extra_body(body: &mut Value, provider: &ProviderConfig) -> Result<()> {
    let extra = provider.extra_body.trim();
    if extra.is_empty() {
        return Ok(());
    }
    let extra = serde_json::from_str::<Value>(extra)
        .with_context(|| format!("invalid extra_body JSON for provider {}", provider.id))?;
    if !extra.is_object() {
        bail!("provider {} extra_body must be a JSON object", provider.id);
    }
    merge_json(body, extra);
    Ok(())
}

/// 深度合并 JSON 对象。
///
/// 参数:
/// - `target`: 被合并的目标 JSON
/// - `patch`: 覆盖来源 JSON
///
/// 返回:
/// - 无
fn merge_json(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_deepseek_thinking() {
        let mut provider = ProviderConfig::default_openai();
        provider.id = "deepseek".to_string();
        provider.thinking_level = "high".to_string();
        let body = apply_provider_body_options(
            json!({"model":"deepseek-chat"}),
            &provider,
            ThinkingProtocol::OpenAiChat,
        )
        .unwrap();

        assert_eq!(body["thinking"], json!({"type":"enabled"}));
        assert_eq!(body["reasoning_effort"], json!("high"));
    }

    #[test]
    fn extra_body_overrides_thinking() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_level = "high".to_string();
        provider.thinking_format = "reasoning".to_string();
        provider.extra_body = r#"{"reasoning":{"effort":"low"}}"#.to_string();
        let body =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
                .unwrap();

        assert_eq!(body["reasoning"], json!({"effort":"low"}));
    }

    #[test]
    fn openai_reasoning_preserves_xhigh_effort() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_level = "xhigh".to_string();
        provider.thinking_format = "reasoning".to_string();
        let body =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
                .unwrap();

        assert_eq!(body["reasoning"], json!({"effort":"xhigh"}));
    }

    #[test]
    fn openai_reasoning_maps_max_to_xhigh_effort() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_level = "max".to_string();
        provider.thinking_format = "openai-chat-reasoning-effort".to_string();
        let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();

        assert_eq!(body["reasoning_effort"], json!("xhigh"));
    }
}
