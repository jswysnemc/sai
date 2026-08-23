/// LLM 客户端共享辅助函数：Responses 流、请求头、Anthropic 工具与错误判定。

/// Responses 流空闲超时：正文阶段过久无新字节时收尾。
///
/// 参数:
/// - `provider_timeout_seconds`: 供应商请求超时配置
///
/// 返回:
/// - 空闲等待上限
fn responses_stream_idle_timeout(provider_timeout_seconds: u64) -> Duration {
    // 1. 默认 8 秒足够覆盖网关间歇；不超过供应商超时的一半
    let half = provider_timeout_seconds.saturating_div(2).max(3);
    Duration::from_secs(half.min(15).max(5))
}

/// 冲刷 Responses 流缓冲中尚未推送的文本。
///
/// 参数:
/// - `content`: 已聚合正文
/// - `content_emitted`: 已推送正文字节数
/// - `reasoning`: 已聚合推理
/// - `reasoning_emitted`: 已推送推理字节数
/// - `on_event`: 流式事件回调
///
/// 返回:
/// - 冲刷结果
fn flush_responses_buffers<F>(
    content: &str,
    content_emitted: &mut usize,
    reasoning: &str,
    reasoning_emitted: &mut usize,
    on_event: &mut F,
) -> Result<()>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    // 直接复用 stream_handlers 内部逻辑不可用（private）；此处补发剩余切片
    if *content_emitted < content.len() {
        let text = content[*content_emitted..].to_string();
        *content_emitted = content.len();
        if !text.is_empty() {
            on_event(ChatStreamEvent::Chunk(ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text,
            }))?;
        }
    }
    if *reasoning_emitted < reasoning.len() {
        let text = reasoning[*reasoning_emitted..].to_string();
        *reasoning_emitted = reasoning.len();
        if !text.is_empty() {
            on_event(ChatStreamEvent::Chunk(ChatStreamChunk {
                kind: ChatStreamKind::Reasoning,
                text,
            }))?;
        }
    }
    Ok(())
}

/// 拆分 Responses 请求的 instructions 与 input 消息。
///
/// 参数:
/// - `messages`: 原始 Chat 消息
///
/// 返回:
/// - (可选 instructions, 剩余消息)
fn split_responses_instructions(messages: Vec<ChatMessage>) -> (Option<String>, Vec<ChatMessage>) {
    let mut instructions = Vec::new();
    let mut rest = Vec::new();
    let mut past_system = false;
    for message in messages {
        if !past_system && message.role == "system" {
            if let Some(text) = message.content.as_ref().map(|c| match c {
                crate::llm::ChatContent::Text(t) => t.clone(),
                crate::llm::ChatContent::Parts(parts) => parts
                    .iter()
                    .filter_map(|p| match p {
                        crate::llm::ChatContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
            }) {
                if !text.trim().is_empty() {
                    instructions.push(text);
                }
            }
            continue;
        }
        past_system = true;
        rest.push(message);
    }
    let joined = instructions.join(
        "

",
    );
    ((!joined.trim().is_empty()).then_some(joined), rest)
}

/// Codex CLI 默认 User-Agent。
const CODEX_CLI_USER_AGENT: &str = "codex_cli_rs/0.144.0";
/// Claude Code CLI 默认 User-Agent。
const CLAUDE_CLI_USER_AGENT: &str = "claude-cli/2.1.113 (external, cli)";
/// 非 Codex / Claude 默认 User-Agent。
const DEFAULT_HTTP_USER_AGENT: &str = "sai/0.1";

/// 构造 Codex Responses 调试用请求头列表。
///
/// 参数:
/// - `api_key`: API Key
/// - `session_id`: 会话 UUID
/// - `user_agent`: 解析后的 User-Agent
///
/// 返回:
/// - 调试/日志用的头列表
fn codex_responses_request_headers(
    api_key: &str,
    session_id: &str,
    user_agent: &str,
) -> Vec<(String, String)> {
    bearer_request_headers(
        api_key,
        &[
            ("User-Agent", user_agent),
            ("originator", "codex_cli_rs"),
            ("OpenAI-Beta", "responses=experimental"),
            ("version", "0.144.0"),
            ("session_id", session_id),
            ("x-client-request-id", session_id),
        ],
    )
}

/// 解析供应商最终 User-Agent。
///
/// 参数:
/// - `provider`: 供应商配置
///
/// 返回:
/// - 自定义 UA；否则 Codex / Claude 风格用对应 CLI UA，其它用 sai 默认 UA
fn resolve_provider_user_agent(provider: &ProviderConfig) -> String {
    let custom = provider.user_agent.trim();
    if !custom.is_empty() {
        return custom.to_string();
    }
    let style = provider.client_style.trim().to_ascii_lowercase();
    if style == "codex" {
        return CODEX_CLI_USER_AGENT.to_string();
    }
    if matches!(style.as_str(), "claude" | "claude-code" | "claude_code") {
        return CLAUDE_CLI_USER_AGENT.to_string();
    }
    // auto 且命中 Claude Code 代理特征时也使用 Claude CLI UA
    if provider_uses_claude_code_style(provider) {
        return CLAUDE_CLI_USER_AGENT.to_string();
    }
    DEFAULT_HTTP_USER_AGENT.to_string()
}

/// 为非 Codex 请求附加 User-Agent。
fn apply_provider_user_agent(
    req: reqwest::RequestBuilder,
    provider: &ProviderConfig,
) -> reqwest::RequestBuilder {
    req.header("User-Agent", resolve_provider_user_agent(provider))
}

/// 附加 Codex Responses 协议头（含可覆盖的 User-Agent）。
fn apply_codex_response_headers(
    req: reqwest::RequestBuilder,
    provider: &ProviderConfig,
    session_id: &str,
) -> reqwest::RequestBuilder {
    let user_agent = resolve_provider_user_agent(provider);
    req.header("User-Agent", user_agent)
        .header("originator", "codex_cli_rs")
        .header("OpenAI-Beta", "responses=experimental")
        .header("version", "0.144.0")
        .header("session_id", session_id)
        .header("Accept", "text/event-stream")
        .header("x-client-request-id", session_id)
}

/// 将供应商自定义头合并进调试头列表。
///
/// 参数:
/// - `headers`: 已有头
/// - `provider`: 供应商配置
///
/// 返回:
/// - 合并后的头列表
fn merge_provider_extra_headers(
    mut headers: Vec<(String, String)>,
    provider: &ProviderConfig,
) -> Vec<(String, String)> {
    for (name, value) in &provider.extra_headers {
        let key = name.trim();
        if key.is_empty() {
            continue;
        }
        // 自定义头覆盖同名项
        if let Some(pos) = headers
            .iter()
            .position(|(existing, _)| existing.eq_ignore_ascii_case(key))
        {
            headers[pos] = (key.to_string(), value.clone());
        } else {
            headers.push((key.to_string(), value.clone()));
        }
    }
    headers
}

/// 向 reqwest 请求附加供应商自定义头。
///
/// 参数:
/// - `req`: 请求构建器
/// - `provider`: 供应商配置
///
/// 返回:
/// - 附带头后的构建器
fn with_provider_extra_headers(
    mut req: reqwest::RequestBuilder,
    provider: &ProviderConfig,
) -> reqwest::RequestBuilder {
    let has_custom_ua = !provider.user_agent.trim().is_empty();
    for (name, value) in &provider.extra_headers {
        let key = name.trim();
        if key.is_empty() {
            continue;
        }
        // 不覆盖 Authorization；专用 user_agent 字段优先于 extra_headers 中的 User-Agent
        if key.eq_ignore_ascii_case("authorization") {
            continue;
        }
        if has_custom_ua && key.eq_ignore_ascii_case("user-agent") {
            continue;
        }
        req = req.header(key, value);
    }
    req
}

/// 判断供应商是否指向官方 Anthropic API。
///
/// 参数:
/// - `provider`: 供应商配置
///
/// 返回:
/// - 仅官方 Anthropic 特征返回 true，Claude 代理不自动切换协议
fn provider_looks_official_anthropic(provider: &ProviderConfig) -> bool {
    provider.uses_official_anthropic_api()
}

/// 判断配置是否可能误把 Claude 当作 OpenAI Chat 协议。
///
/// 参数:
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - 需要提示协议配置时返回英文提示，否则返回空字符串
fn claude_protocol_hint(provider: &ProviderConfig) -> &'static str {
    let protocol = provider.protocol.trim();
    let model = provider.default_model.to_ascii_lowercase();
    let claude_related = model.contains("claude")
        || provider.id.to_ascii_lowercase().contains("claude")
        || provider
            .display_name
            .to_ascii_lowercase()
            .contains("claude");
    if claude_related
        && !provider_looks_official_anthropic(provider)
        && matches!(protocol, "" | "auto" | "openai-chat")
    {
        return "\nHint: Claude proxies that require Anthropic Messages should set protocol=anthropic (and client_style=claude for Claude Code 1M context headers); official Anthropic uses protocol=anthropic and base_url=https://api.anthropic.com/v1.";
    }
    ""
}

/// 判断 Anthropic 错误是否允许移除 thinking 后重试。
///
/// 参数:
/// - `status`: HTTP 状态码
/// - `body`: 服务端错误响应正文
///
/// 返回:
/// - 服务端明确拒绝 thinking 参数时返回 true
fn anthropic_thinking_unsupported(status: u16, body: &str) -> bool {
    thinking_parameter_rejected(status, body)
}

/// 判断 OpenAI Chat 兼容错误是否允许移除 thinking 后重试。
///
/// 参数:
/// - `status`: HTTP 状态码
/// - `body`: 服务端错误响应正文
///
/// 返回:
/// - 服务端明确拒绝 thinking 参数时返回 true
fn openai_chat_thinking_unsupported(status: u16, body: &str) -> bool {
    thinking_parameter_rejected(status, body)
}

/// 判断错误正文是否为 thinking 参数校验失败。
///
/// 参数:
/// - `status`: HTTP 状态码
/// - `body`: 服务端错误响应正文
///
/// 返回:
/// - 明确与 thinking 校验相关时返回 true
fn thinking_parameter_rejected(status: u16, body: &str) -> bool {
    if !matches!(status, 400 | 422) {
        return false;
    }
    let body = body.to_ascii_lowercase();
    if !body.contains("thinking") {
        return false;
    }
    [
        "unsupported",
        "not supported",
        "unknown",
        "invalid",
        "unrecognized",
        "validation",
        "unmarshal",
        "must be a boolean",
        "must be",
        "cannot unmarshal",
        "bad_response_status_code",
    ]
    .iter()
    .any(|marker| body.contains(marker))
}

/// 按模型配置处理 Anthropic 网页搜索工具名称冲突。
///
/// 参数:
/// - `provider`: 当前供应商配置
/// - `tools`: 当前可用工具
///
/// 返回:
/// - 已隐藏或更名本地网页搜索工具的列表
fn prepare_anthropic_tools(
    provider: &ProviderConfig,
    tools: Vec<ToolDefinition>,
) -> Vec<ToolDefinition> {
    match provider.model_web_search_tool_mode_for(&provider.default_model) {
        WEB_SEARCH_TOOL_MODE_HIDE => tools
            .into_iter()
            .filter(|tool| tool.function.name != "web_search")
            .collect(),
        WEB_SEARCH_TOOL_MODE_RENAME => tools
            .into_iter()
            .map(|mut tool| {
                if tool.function.name == "web_search" {
                    tool.function.name = "sai_web_search".to_string();
                }
                tool
            })
            .collect(),
        _ => tools,
    }
}

/// DeepSeek 思考模式要求带 `tool_calls` 的 assistant 同时携带 `reasoning_content`。
/// flash 一类模型经常直接出工具、不写思考；缺字段时补这个占位，避免服务端 400。
const DEEPSEEK_TOOL_REASONING_PLACEHOLDER: &str = ".";

/// 【协议】【思考回传】根据供应商规则准备历史思考内容。
///
/// 普通兼容网关移除 `reasoning_content`，避免因未知字段拒绝请求；DeepSeek
/// 与显式开启 Preserved Thinking 的供应商保持原消息。DeepSeek 思考模式开启时，
/// 带 `tool_calls` 却缺少思考字段的 assistant 会补占位思考，而不是丢掉整段
/// assistant/tool 历史——丢掉当前轮工具结果会让模型看不见输出，从而无限重发。
/// 关闭思考后保留普通工具历史，不再应用该限制。
///
/// 参数:
/// - `messages`: 待发送的消息序列
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - 已按供应商规则处理的消息序列
fn apply_preserved_thinking(
    messages: Vec<ChatMessage>,
    provider: &ProviderConfig,
) -> Vec<ChatMessage> {
    if !should_preserve_reasoning(provider) {
        return messages
            .into_iter()
            .map(|message| ChatMessage {
                reasoning_content: None,
                ..message
            })
            .collect();
    }
    if !deepseek_requires_tool_reasoning(provider) {
        return messages;
    }
    messages
        .into_iter()
        .map(|mut message| {
            // 1. 缺思考的工具调用补占位，保证 DeepSeek 收下完整 tool 结果
            if message.role == "assistant"
                && message.tool_calls.is_some()
                && message
                    .reasoning_content
                    .as_deref()
                    .is_none_or(str::is_empty)
            {
                message.reasoning_content = Some(DEEPSEEK_TOOL_REASONING_PLACEHOLDER.to_string());
            }
            message
        })
        .collect()
}
