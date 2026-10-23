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
    // 先落到当前模型实际支持的档位：配置里留着的旧等级换模型后仍会发出，
    // 服务端拒绝的是整个请求，而不只是这一个字段
    let requested = normalized_level(&provider.thinking_level);
    let resolved = crate::config::resolve_thinking_level(
        provider.model_thinking_levels_for(&provider.default_model),
        requested,
    );
    let level = resolved.as_str();
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
        DEEPSEEK_RESPONSES_FORMAT => apply_deepseek_responses_reasoning(body, level),
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
    if let Some(format) = deepseek_responses_format_if_enabled(provider, protocol, configured) {
        return format;
    }
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
    crate::config::is_deepseek_like_model(provider, &provider.default_model)
}

/// 判断 DeepSeek 当前是否要求工具子轮回传思考内容。
///
/// 参数:
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - DeepSeek 思考模式开启时返回 true
pub(crate) fn deepseek_requires_tool_reasoning(provider: &ProviderConfig) -> bool {
    is_deepseek_provider(provider) && normalized_level(&provider.thinking_level) != "none"
}

/// 判断当前供应商是否需要回传历史思考内容。
///
/// 参数:
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - 需要保留 `reasoning_content` 时返回 true
pub(crate) fn should_preserve_reasoning(provider: &ProviderConfig) -> bool {
    provider.preserve_thinking || deepseek_requires_tool_reasoning(provider)
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
        // DeepSeek 服务端把 xhigh 与 high 视为同一档，升级到 max 会超出用户所选档位
        "max" => "max",
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
/// 对象逐键递归合并；数组按追加处理，不整体替换。
///
/// 数组替换会让 extra_body 里的 `tools` 冲掉运行期已经装配好的函数工具——
/// 用户想补一个 `{"type": "web_search"}`，结果丢掉了全部本地工具。
/// 追加语义下补充项与既有项共存，重复项按内容去重。
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
        (Value::Array(target), Value::Array(patch)) => {
            for value in patch {
                if !target.contains(&value) {
                    target.push(value);
                }
            }
        }
        (target, patch) => {
            *target = patch;
        }
    }
}

/// DeepSeek Responses 协议思考参数修正开关。
const FEATURE_DEEPSEEK_RESPONSES_REASONING_ENABLED: bool = true;

/// DeepSeek Responses 协议思考参数格式标识。
const DEEPSEEK_RESPONSES_FORMAT: &str = "deepseek-responses-reasoning";

/// 【协议】【DeepSeek】计算 Responses 协议下的思考参数格式。
///
/// DeepSeek Responses 接口只认嵌套的 `reasoning.effort`，Chat 格式的顶层
/// `thinking` / `reasoning_effort` 会被静默忽略，实际生效的仍是请求体里的默认档位。
///
/// 参数:
/// - `provider`: 当前 provider 配置
/// - `protocol`: 当前请求协议
/// - `configured`: 已去除首尾空白的 thinking_format 配置
///
/// 返回:
/// - 命中 DeepSeek Responses 时返回格式标识，否则返回 `None` 沿用原有判定
fn deepseek_responses_format_if_enabled(
    provider: &ProviderConfig,
    protocol: ThinkingProtocol,
    configured: &str,
) -> Option<&'static str> {
    if !FEATURE_DEEPSEEK_RESPONSES_REASONING_ENABLED {
        return None;
    }
    if protocol != ThinkingProtocol::OpenAiResponses || configured == "disabled" {
        return None;
    }
    is_deepseek_provider(provider).then_some(DEEPSEEK_RESPONSES_FORMAT)
}

/// 【协议】【DeepSeek】写入 Responses 协议的思考参数。
///
/// 只覆盖 `reasoning.effort`，保留 summary 等同级字段。
///
/// 参数:
/// - `body`: 待修改的请求体
/// - `level`: 思考等级
///
/// 返回:
/// - 无
fn apply_deepseek_responses_reasoning(body: &mut Value, level: &str) {
    body["reasoning"]["effort"] = json!(deepseek_responses_effort(level));
}

/// 映射为 DeepSeek Responses 支持的 effort。
///
/// `none` 必须原样发出才能关闭思考；通用映射给出的 `minimal` 会被服务端当作 `low`。
/// DeepSeek 服务端把 xhigh 与 high 视为同一档，因此 xhigh 发送 high。
///
/// 参数:
/// - `level`: 思考等级
///
/// 返回:
/// - none / low / high / max 四档之一
fn deepseek_responses_effort(level: &str) -> &'static str {
    match level {
        "none" => "none",
        "low" => "low",
        "max" => "max",
        _ => "high",
    }
}

#[cfg(test)]
#[path = "thinking/tests.rs"]
mod tests;
