/// Claude Code 客户端模拟：请求头、URL 与 Messages 请求体形态。
///
/// 对标抓包与 Claude CLI 特征：
/// - `User-Agent: claude-cli/<ver> (external, cli)`
/// - `anthropic-beta` 含 `claude-code-20250219` 与 `context-1m-2025-08-07`
/// - `/v1/messages?beta=true`
/// - `system` 为 text block 数组，并带 Claude Code 身份前缀
/// - `metadata.user_id` 为设备 / 会话 JSON 字符串


/// Claude Code 默认 CLI 版本（与抓包 2.1.113 对齐）。
const CLAUDE_CLI_VERSION: &str = "2.1.113";

/// Claude Code 基础 beta（不含 1M）。
const CLAUDE_CODE_BETA_BASE: &str = concat!(
    "claude-code-20250219,",
    "interleaved-thinking-2025-05-14,",
    "redact-thinking-2026-02-12,",
    "context-management-2025-06-27,",
    "prompt-caching-scope-2026-01-05,",
    "effort-2025-11-24"
);

/// Claude Code 1M 上下文 beta 标记。
const CLAUDE_CODE_CONTEXT_1M_BETA: &str = "context-1m-2025-08-07";

/// Claude Code 系统身份文案。
const CLAUDE_CODE_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// 判断是否应按 Claude Code 通道形态发 Anthropic Messages。
///
/// 参数:
/// - `model`: 模型名
/// - `base_url`: 供应商 base_url
/// - `client_style`: 供应商客户端模拟（auto/default/codex/claude）
/// - `official_anthropic`: 是否官方 api.anthropic.com
///
/// 返回:
/// - 需要 Claude Code 请求头与 body 形态时 true
fn prefers_claude_code_shape(
    model: &str,
    base_url: &str,
    client_style: &str,
    official_anthropic: bool,
) -> bool {
    let style = client_style.trim().to_ascii_lowercase();
    // 1. 显式开关
    if matches!(style.as_str(), "claude" | "claude-code" | "claude_code") {
        return true;
    }
    if matches!(style.as_str(), "default" | "codex") {
        return false;
    }
    // 2. auto：官方 Anthropic 保持标准 Messages；第三方 Claude 代理启用模拟
    if official_anthropic {
        return false;
    }
    let model = model.to_ascii_lowercase();
    let url = base_url.to_ascii_lowercase();
    let claude_model = model.contains("claude");
    let proxy_hint = url.contains("fcapp.run")
        || url.contains("new-api")
        || url.contains("one-api")
        || url.contains("oneapi")
        || url.contains("openrouter")
        || url.contains("anyrouter")
        || url.contains("claude");
    claude_model && proxy_hint
}

/// 构造 Claude Code `anthropic-beta` 头。
///
/// 参数:
/// - `enable_1m`: 是否附加 1M 上下文 beta
///
/// 返回:
/// - beta 头字符串
fn claude_code_beta_header(enable_1m: bool) -> String {
    if enable_1m {
        // 1. 1M 标记紧随 claude-code 门禁，满足代理启用条件
        format!(
            "claude-code-20250219,{CLAUDE_CODE_CONTEXT_1M_BETA},{}",
            CLAUDE_CODE_BETA_BASE.trim_start_matches("claude-code-20250219,")
        )
    } else {
        CLAUDE_CODE_BETA_BASE.to_string()
    }
}

/// 构造 Claude Code Messages 调试/日志用请求头。
///
/// 参数:
/// - `api_key`: API Key
/// - `session_id`: 会话 UUID
/// - `user_agent`: 解析后的 User-Agent
/// - `enable_1m`: 是否启用 1M 上下文 beta
///
/// 返回:
/// - 头列表（含 x-api-key）
fn claude_code_request_headers(
    api_key: &str,
    session_id: &str,
    user_agent: &str,
    enable_1m: bool,
) -> Vec<(String, String)> {
    vec![
        ("x-api-key".to_string(), api_key.to_string()),
        ("anthropic-version".to_string(), "2023-06-01".to_string()),
        (
            "anthropic-beta".to_string(),
            claude_code_beta_header(enable_1m),
        ),
        (
            "anthropic-dangerous-direct-browser-access".to_string(),
            "true".to_string(),
        ),
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Accept".to_string(), "text/event-stream".to_string()),
        ("User-Agent".to_string(), user_agent.to_string()),
        ("x-app".to_string(), "cli".to_string()),
        (
            "x-claude-code-session-id".to_string(),
            session_id.to_string(),
        ),
        ("x-stainless-lang".to_string(), "js".to_string()),
        (
            "x-stainless-package-version".to_string(),
            "0.81.0".to_string(),
        ),
        ("x-stainless-runtime".to_string(), "node".to_string()),
        (
            "x-stainless-runtime-version".to_string(),
            "v24.3.0".to_string(),
        ),
        ("x-stainless-retry-count".to_string(), "0".to_string()),
    ]
}

/// 将 Anthropic Messages URL 调整为 Claude Code 形态（附加 beta=true）。
///
/// 参数:
/// - `url`: 原始 `/messages` URL
///
/// 返回:
/// - 带 `beta=true` 的 URL
fn claude_code_messages_url(url: &str) -> String {
    if url.contains("beta=") {
        return url.to_string();
    }
    if url.contains('?') {
        format!("{url}&beta=true")
    } else {
        format!("{url}?beta=true")
    }
}

/// 将请求体重塑为 Claude Code Messages 形态。
///
/// 参数:
/// - `body`: 已序列化的 Anthropic 请求体
/// - `session_id`: 会话 UUID
/// - `thinking_level`: provider 思考等级
///
/// 返回:
/// - 无；原地修改 body
fn apply_claude_code_body_shape(body: &mut Value, session_id: &str, thinking_level: &str) {
    // 1. system 改为 text block 数组，并注入 billing / 身份前缀
    let original_system = body
        .get("system")
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Array(items) => {
                let joined = items
                    .iter()
                    .filter_map(|item| {
                        item.get("text")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .collect::<Vec<_>>()
                    .join("
");
                Some(joined)
            }
            _ => None,
        })
        .unwrap_or_default();
    let mut system_blocks = vec![
        json!({
            "type": "text",
            "text": claude_billing_header_text(),
        }),
        json!({
            "type": "text",
            "text": CLAUDE_CODE_IDENTITY,
        }),
    ];
    if !original_system.trim().is_empty() {
        system_blocks.push(json!({
            "type": "text",
            "text": original_system,
        }));
    }
    body["system"] = Value::Array(system_blocks);

    // 2. metadata.user_id：设备指纹 + 会话 id
    body["metadata"] = json!({
        "user_id": claude_metadata_user_id(session_id),
    });

    // 3. 思考：Claude Code 使用 adaptive + output_config.effort
    let level = thinking_level.trim().to_ascii_lowercase();
    if level.is_empty() || level == "auto" || level == "none" || level == "disabled" {
        if let Some(object) = body.as_object_mut() {
            object.remove("thinking");
            if let Some(Value::Object(cfg)) = object.get_mut("output_config") {
                cfg.remove("effort");
                if cfg.is_empty() {
                    object.remove("output_config");
                }
            }
        }
    } else {
        body["thinking"] = json!({ "type": "adaptive" });
        let effort = claude_effort_from_level(&level);
        match body.get_mut("output_config") {
            Some(Value::Object(cfg)) => {
                cfg.insert("effort".to_string(), json!(effort));
            }
            _ => {
                body["output_config"] = json!({ "effort": effort });
            }
        }
    }
}

/// 生成 billing header 系统块文本。
///
/// 返回:
/// - `x-anthropic-billing-header: ...` 行
fn claude_billing_header_text() -> String {
    // cch 使用版本派生短指纹，避免每次请求随机导致缓存失效
    let digest = Sha256::digest(format!("sai-claude-cli-{CLAUDE_CLI_VERSION}").as_bytes());
    let cch = hex::encode(&digest[..3]);
    format!(
        "x-anthropic-billing-header: cc_version={CLAUDE_CLI_VERSION}.sai; cc_entrypoint=cli; cch={cch};"
    )
}

/// 构造 Claude Code metadata.user_id JSON 字符串。
///
/// 参数:
/// - `session_id`: 会话 UUID
///
/// 返回:
/// - 内嵌 JSON 字符串
fn claude_metadata_user_id(session_id: &str) -> String {
    let device = hex::encode(Sha256::digest(
        format!("sai-device-{session_id}").as_bytes(),
    ));
    json!({
        "device_id": device,
        "account_uuid": "",
        "session_id": session_id,
    })
    .to_string()
}

/// 将 thinking_level 映射为 Claude Code effort。
///
/// 参数:
/// - `level`: 已小写的思考等级
///
/// 返回:
/// - effort 字符串
fn claude_effort_from_level(level: &str) -> &'static str {
    match level {
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        "xhigh" | "max" => "xhigh",
        _ => "high",
    }
}

/// 判断供应商当前是否启用 Claude Code 模拟。
///
/// 参数:
/// - `provider`: 供应商配置
///
/// 返回:
/// - 启用时 true
fn provider_uses_claude_code_style(provider: &ProviderConfig) -> bool {
    prefers_claude_code_shape(
        &provider.default_model,
        &provider.base_url,
        &provider.client_style,
        provider.uses_official_anthropic_api(),
    )
}
