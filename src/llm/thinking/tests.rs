use super::*;

/// 验证 extra_body 里的工具补充项不会冲掉已装配的函数工具。
///
/// DeepSeek 的 Responses API 支持在 tools 里混用 function 与 web_search。
/// 数组若按整体替换处理，用户补一个 web_search 就会丢掉全部本地工具。
#[test]
fn extra_body_appends_tools_instead_of_replacing_them() {
    let mut body = serde_json::json!({
        "model": "deepseek-chat",
        "tools": [
            {"type": "function", "name": "read_file"},
            {"type": "function", "name": "run_command"}
        ]
    });

    merge_json(
        &mut body,
        serde_json::json!({"tools": [{"type": "web_search"}]}),
    );

    let tools = body["tools"].as_array().expect("tools 应当仍是数组");
    assert_eq!(tools.len(), 3, "补充项应当与既有工具共存");
    assert!(tools.contains(&serde_json::json!({"type": "function", "name": "read_file"})));
    assert!(tools.contains(&serde_json::json!({"type": "web_search"})));
}

/// 验证重复的数组项不会被追加两次。
#[test]
fn merging_an_identical_array_item_is_idempotent() {
    let mut body = serde_json::json!({"tools": [{"type": "web_search"}]});

    merge_json(
        &mut body,
        serde_json::json!({"tools": [{"type": "web_search"}]}),
    );

    assert_eq!(body["tools"].as_array().unwrap().len(), 1);
}

/// 验证嵌套对象仍然逐键合并。
#[test]
fn nested_objects_merge_key_by_key() {
    let mut body = serde_json::json!({"thinking": {"type": "enabled", "budget": 1024}});

    merge_json(&mut body, serde_json::json!({"thinking": {"budget": 4096}}));

    assert_eq!(body["thinking"]["type"], "enabled", "未提及的键应当保留");
    assert_eq!(body["thinking"]["budget"], 4096);
}

/// 验证标量仍然按覆盖处理。
#[test]
fn scalars_are_overwritten() {
    let mut body = serde_json::json!({"temperature": 1.0});

    merge_json(&mut body, serde_json::json!({"temperature": 0.2}));

    assert_eq!(body["temperature"], 0.2);
}

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

/// 【协议】【DeepSeek】验证关闭思考后不再回传历史思考。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn deepseek_disabled_thinking_does_not_preserve_reasoning() {
    let mut provider = ProviderConfig::default_openai();
    provider.id = "deepseek".to_string();
    provider.thinking_level = "none".to_string();

    assert!(!should_preserve_reasoning(&provider));
}

#[test]
fn extra_body_overrides_thinking() {
    let mut provider = ProviderConfig::default_openai();
    provider.thinking_level = "high".to_string();
    provider.thinking_format = "reasoning".to_string();
    provider.extra_body = r#"{"reasoning":{"effort":"low"}}"#.to_string();
    let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
        .unwrap();

    assert_eq!(body["reasoning"], json!({"effort":"low"}));
}

#[test]
fn openai_reasoning_preserves_xhigh_effort() {
    let mut provider = ProviderConfig::default_openai();
    provider.thinking_level = "xhigh".to_string();
    provider.thinking_format = "reasoning".to_string();
    let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
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
    let max =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();
    provider.thinking_level = "xhigh".to_string();
    let xhigh =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

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
    let high =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();
    provider.thinking_level = "low".to_string();
    let low =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

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
    assert_eq!(
        body["reasoning"]["summary"],
        json!("auto"),
        "summary 不应被覆盖"
    );
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
        let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
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
    let body =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

    assert_eq!(body["thinking"], json!({"type": "disabled"}));
}

/// 【协议】【思考等级】验证请求等级落到模型实际支持的档位。
///
/// 只支持 high/max 的模型收到 xhigh 时必须降到 high：
/// 原样发出会让服务端拒绝整个请求，而不只是忽略这个字段。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn unsupported_level_is_downgraded_to_a_supported_one() {
    let mut provider = ProviderConfig::default_openai();
    provider.thinking_format = "openai-chat-reasoning-effort".to_string();
    provider.default_model = "some-model".to_string();
    provider
        .set_model_thinking_levels_for("some-model", vec!["high".to_string(), "max".to_string()]);
    provider.thinking_level = "xhigh".to_string();

    let body =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

    assert_eq!(body["reasoning_effort"], json!("high"));
}

/// 【协议】【思考等级】验证未记录支持范围时不改写请求等级。
///
/// 目录没覆盖到的模型按全部可用处理，否则新模型一律被降级。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn unknown_model_support_keeps_the_requested_level() {
    let mut provider = ProviderConfig::default_openai();
    provider.thinking_format = "openai-chat-reasoning-effort".to_string();
    provider.thinking_level = "xhigh".to_string();

    let body =
        apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat).unwrap();

    assert_eq!(body["reasoning_effort"], json!("xhigh"));
}

/// 【协议】【DeepSeek】验证 Responses 协议只写 `reasoning.effort`。
///
/// 经网关转发的 DeepSeek 模型走 Codex 通道时，默认 effort 为 low；
/// 若再写入 Chat 格式字段，界面等级不会生效，请求体出现三套互相矛盾的思考参数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn deepseek_responses_uses_nested_reasoning_effort() {
    let mut provider = ProviderConfig::default_openai();
    provider.id = "magpie".to_string();
    provider.default_model = "clinepass/cline-pass/deepseek-v4.1-flash-1017".to_string();
    provider.thinking_level = "xhigh".to_string();

    let body = apply_provider_body_options(
        json!({ "reasoning": { "effort": "low", "summary": "auto" } }),
        &provider,
        ThinkingProtocol::OpenAiResponses,
    )
    .unwrap();

    assert_eq!(
        body["reasoning"],
        json!({ "effort": "high", "summary": "auto" })
    );
    assert!(
        body.get("thinking").is_none(),
        "Responses 协议不应写 thinking"
    );
    assert!(
        body.get("reasoning_effort").is_none(),
        "Responses 协议不应写顶层 reasoning_effort"
    );
}

/// 【协议】【DeepSeek】验证 Responses 协议关闭思考时发送 none。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn deepseek_responses_none_disables_thinking() {
    let mut provider = ProviderConfig::default_openai();
    provider.id = "deepseek".to_string();
    provider.thinking_level = "none".to_string();

    let body = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
        .unwrap();

    assert_eq!(body["reasoning"]["effort"], json!("none"));
    assert!(body.get("thinking").is_none());
}

/// 【协议】【DeepSeek】验证 xhigh 发送 high，max 保持 max。
///
/// DeepSeek 服务端把 xhigh 与 high 视为同一档，升级到 max 会超出用户所选档位。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn deepseek_xhigh_maps_to_high_on_both_protocols() {
    let mut provider = ProviderConfig::default_openai();
    provider.id = "deepseek".to_string();

    for (level, expected) in [("xhigh", "high"), ("max", "max")] {
        provider.thinking_level = level.to_string();
        let chat = apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiChat)
            .unwrap();
        let responses =
            apply_provider_body_options(json!({}), &provider, ThinkingProtocol::OpenAiResponses)
                .unwrap();

        assert_eq!(chat["reasoning_effort"], json!(expected), "Chat {level}");
        assert_eq!(
            responses["reasoning"]["effort"],
            json!(expected),
            "Responses {level}"
        );
    }
}
