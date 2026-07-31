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
        "boolean" => {
            body["thinking"] = json!(level != "none");
        }
        "object" => apply_generic_thinking_object(body, level),
        "type-object" => apply_type_thinking(body, level),
        "chat-auto" => apply_chat_auto_thinking(body, level),
        "deepseek-thinking" => apply_deepseek_thinking(body, level),
        "moonshot-thinking" => apply_moonshot_thinking(body, level, provider.preserve_thinking),
        "openai-chat-reasoning-effort" => {
            if level != "none" {
                body["reasoning_effort"] = json!(reasoning_effort(level));
            }
        }
        "reasoning" => {
            // 只覆盖 effort：调用方可能已经设置了 summary 等同级字段，
            // 整体赋值会把它们一并丢弃
            body["reasoning"]["effort"] = json!(reasoning_effort(level));
        }
        "anthropic-thinking" => apply_anthropic_thinking(body, level),
        _ => apply_type_thinking(body, level),
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
            "boolean" => "boolean",
            "object" => "object",
            "type-object" | "type" => "type-object",
            "deepseek-thinking" => "deepseek-thinking",
            "moonshot-thinking" => "moonshot-thinking",
            "openai-chat-reasoning-effort" => "openai-chat-reasoning-effort",
            "reasoning" => "reasoning",
            "anthropic-thinking" => "anthropic-thinking",
            "disabled" => "disabled",
            // 未知格式不再回退到字符串，避免多数 OpenAI 兼容网关 400
            _ => "type-object",
        };
    }
    match protocol {
        ThinkingProtocol::OpenAiResponses => "reasoning",
        ThinkingProtocol::Anthropic => "anthropic-thinking",
        // OpenAI Chat 兼容网关普遍要求 thinking 为 bool 或 {type:enabled|disabled|adaptive}，
        // 但仅有开关无法表达等级，因此同时写出 reasoning_effort
        ThinkingProtocol::OpenAiChat => "chat-auto",
    }
}

/// 判断 provider 是否为 DeepSeek 兼容供应商。
///
/// 参数:
/// - `provider`: 当前 provider 配置
///
/// 返回:
/// - 是否匹配 DeepSeek
pub(crate) fn is_deepseek_provider(provider: &ProviderConfig) -> bool {
    let id = provider.id.to_ascii_lowercase();
    let base_url = provider.base_url.to_ascii_lowercase();
    let model = provider.default_model.to_ascii_lowercase();
    id.contains("deepseek") || base_url.contains("deepseek") || model.contains("deepseek")
}

/// 判断当前供应商是否需要回传历史思考内容。
///
/// 参数:
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - 需要保留 `reasoning_content` 时返回 true
pub(crate) fn should_preserve_reasoning(provider: &ProviderConfig) -> bool {
    provider.preserve_thinking || is_deepseek_provider(provider)
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
        "xhigh" => "xhigh",
        // 已有模型支持 max，不再折叠到 xhigh
        "max" => "max",
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

/// 写入 `{ "type": "enabled" | "disabled" }` 形式的 thinking。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_type_thinking(body: &mut Value, level: &str) {
    if level == "none" {
        body["thinking"] = json!({ "type": "disabled" });
        return;
    }
    body["thinking"] = json!({ "type": "enabled" });
}

/// 写入 OpenAI Chat 自动格式的思考参数。
///
/// 兼容网关普遍要求 `thinking` 为开关对象，但开关无法表达等级——只写开关时
/// low 与 max 会产生完全相同的请求体，界面上的等级设置形同失效。因此在开关
/// 之外同时写出 chat 协议通用的 `reasoning_effort`，与 DeepSeek 分支一致。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_chat_auto_thinking(body: &mut Value, level: &str) {
    if level == "none" {
        body["thinking"] = json!({ "type": "disabled" });
        if let Some(object) = body.as_object_mut() {
            object.remove("reasoning_effort");
        }
        return;
    }
    body["thinking"] = json!({ "type": "enabled" });
    body["reasoning_effort"] = json!(reasoning_effort(level));
}

/// 写入 Moonshot（Kimi）OpenAI 兼容思考参数。
///
/// Kimi 的思考等级只接受 low / high / max 三档，其余等级必须先折叠，
/// 否则服务端拒绝请求。多轮回传历史思考需要 `thinking.keep = "all"`，
/// kimi-k2.7-code 一类模型强制要求该行为。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
/// - `preserve`: 是否开启 Preserved Thinking
///
/// 返回:
/// - 无
fn apply_moonshot_thinking(body: &mut Value, level: &str, preserve: bool) {
    if level == "none" {
        body["thinking"] = json!({ "type": "disabled" });
        if let Some(object) = body.as_object_mut() {
            object.remove("reasoning_effort");
        }
        return;
    }
    body["thinking"]["type"] = json!("enabled");
    if preserve {
        body["thinking"]["keep"] = json!("all");
    }
    body["reasoning_effort"] = json!(moonshot_effort(level));
}

/// 映射为 Moonshot 支持的 reasoning_effort。
///
/// 参数:
/// - `level`: 思考等级
///
/// 返回:
/// - low / high / max 三档之一
fn moonshot_effort(level: &str) -> &'static str {
    match level {
        "low" => "low",
        "max" | "xhigh" => "max",
        // medium 与 high 都落到 high：Kimi 不提供中间档
        _ => "high",
    }
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

    /// 【协议】【DeepSeek】验证 DeepSeek 默认回传历史思考。
    ///
    /// 含工具调用的 DeepSeek 请求必须完整回传上一子轮的 `reasoning_content`。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_preserves_reasoning_history_by_default() {
        let mut provider = ProviderConfig::default_openai();
        provider.id = "deepseek".to_string();

        assert!(should_preserve_reasoning(&provider));
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

    /// 【协议】【思考等级】验证 max 保留为独立等级，不再折叠到 xhigh。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn openai_reasoning_keeps_max_effort_distinct() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_format = "openai-chat-reasoning-effort".to_string();

        provider.thinking_level = "max".to_string();
        let max = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();
        provider.thinking_level = "xhigh".to_string();
        let xhigh = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();

        assert_eq!(max["reasoning_effort"], json!("max"));
        assert_eq!(xhigh["reasoning_effort"], json!("xhigh"));
    }

    /// 【协议】【思考等级】验证 chat 自动格式在开关之外带上等级。
    ///
    /// 仅写 thinking 开关时 low 与 max 的请求体完全相同，界面设置形同失效。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn openai_chat_auto_carries_reasoning_effort() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_format = "auto".to_string();

        provider.thinking_level = "high".to_string();
        let high = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();
        provider.thinking_level = "low".to_string();
        let low = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();

        assert_eq!(high["thinking"], json!({"type": "enabled"}));
        assert_eq!(high["reasoning_effort"], json!("high"));
        assert_eq!(low["reasoning_effort"], json!("low"));
        assert_ne!(high, low, "不同等级必须产生不同请求体");
    }

    /// 【协议】【思考等级】验证 responses 协议只覆盖 effort，保留同级字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn openai_responses_preserves_sibling_reasoning_fields() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_level = "high".to_string();
        let body = apply_provider_body_options(
            json!({ "reasoning": { "summary": "auto" } }),
            &provider,
            ThinkingProtocol::OpenAiResponses,
        )
        .unwrap();

        assert_eq!(body["reasoning"]["effort"], json!("high"));
        assert_eq!(body["reasoning"]["summary"], json!("auto"), "summary 不应被覆盖");
    }

    /// 【协议】【Moonshot】验证等级折叠到 low/high/max 三档。
    ///
    /// Kimi 只接受这三档，发送 medium/xhigh 会被服务端拒绝。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn moonshot_folds_levels_into_three_tiers() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_format = "moonshot-thinking".to_string();

        for (level, expected) in [
            ("low", "low"),
            ("medium", "high"),
            ("high", "high"),
            ("xhigh", "max"),
            ("max", "max"),
        ] {
            provider.thinking_level = level.to_string();
            let body =
                apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
                    .unwrap();
            assert_eq!(
                body["reasoning_effort"],
                json!(expected),
                "{level} 应折叠为 {expected}"
            );
            assert_eq!(body["thinking"]["type"], json!("enabled"));
        }
    }

    /// 【协议】【Moonshot】验证 Preserved Thinking 只在开启时写出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn moonshot_keeps_history_only_when_enabled() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_format = "moonshot-thinking".to_string();
        provider.thinking_level = "high".to_string();

        let without =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();
        assert!(
            without["thinking"].get("keep").is_none(),
            "未开启时不应发送 keep"
        );

        provider.preserve_thinking = true;
        let with =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();
        assert_eq!(with["thinking"]["keep"], json!("all"));
    }

    /// 【协议】【Moonshot】验证关闭思考时移除等级字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn moonshot_disables_thinking_without_effort() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_format = "moonshot-thinking".to_string();
        provider.thinking_level = "none".to_string();
        provider.preserve_thinking = true;

        let body =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

        assert_eq!(body["thinking"], json!({ "type": "disabled" }));
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn openai_chat_none_disables_type_object_thinking() {
        let mut provider = ProviderConfig::default_openai();
        provider.thinking_level = "none".to_string();
        let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();

        assert_eq!(body["thinking"], json!({"type": "disabled"}));
    }
}
