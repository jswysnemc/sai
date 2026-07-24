use super::tool_call_stream::ToolCallProgressTracker;
use super::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamEvent, ChatStreamKind, ToolCall,
    ToolCallFunction, ToolCallStreamProgress, ToolDefinition, Usage,
};
use crate::config::{AppConfig, ProviderConfig};
use crate::config::{WEB_SEARCH_TOOL_MODE_HIDE, WEB_SEARCH_TOOL_MODE_RENAME};
use crate::i18n::text as t;
use crate::llm::http_debug::{
    anthropic_request_headers, bearer_request_headers, HttpDebugConfig, HttpDebugRecorder,
};
use crate::llm::thinking::{apply_provider_body_options, ThinkingProtocol};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderProtocol {
    Auto,
    OpenAiChat,
    OpenAiResponses,
    Anthropic,
}

impl ProviderProtocol {
    fn from_provider(provider: &ProviderConfig) -> Result<Self> {
        match provider.protocol.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "openai-chat" => Ok(Self::OpenAiChat),
            "openai-responses" => Ok(Self::OpenAiResponses),
            "anthropic" | "anthropic-messages" | "messages" | "claude" | "claude-code"
            | "claude-messages" => Ok(Self::Anthropic),
            protocol => bail!("unsupported provider protocol: {protocol}"),
        }
    }
}

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    client: Client,
    provider: ProviderConfig,
    api_key: String,
    /// 可选 HTTP 调试落盘配置（`SAI_DEBUG_HTTP`）
    http_debug: Option<HttpDebugConfig>,
}

impl OpenAiCompatibleClient {
    pub fn from_config(config: &AppConfig, paths: &SaiPaths) -> Result<Self> {
        let provider = config.provider(None)?;
        Self::new(provider, config, paths)
    }

    /// 返回客户端绑定的 provider id。
    pub fn provider_id(&self) -> &str {
        &self.provider.id
    }

    /// 返回客户端绑定的 provider 显示名。
    pub fn provider_name(&self) -> &str {
        if self.provider.display_name.trim().is_empty() {
            &self.provider.id
        } else {
            &self.provider.display_name
        }
    }

    /// 返回客户端当前默认模型。
    pub fn model(&self) -> &str {
        &self.provider.default_model
    }

    pub fn new(provider: &ProviderConfig, _config: &AppConfig, paths: &SaiPaths) -> Result<Self> {
        if provider.default_model.trim().is_empty() {
            bail!(
                "{}: {}",
                t(
                    "provider has no active model; select a model before chatting",
                    "provider 没有当前模型；请先选择模型再聊天",
                ),
                provider.id
            );
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(provider.timeout_seconds.clamp(5, 30)))
            .build()?;
        let api_key = provider.resolved_api_key(paths)?;
        Ok(Self {
            client,
            provider: provider.clone(),
            api_key,
            http_debug: HttpDebugConfig::from_env(paths),
        })
    }

    /// 在调试开启时开始一次请求记录。
    ///
    /// 参数:
    /// - `method`: HTTP 方法
    /// - `url`: 请求 URL
    /// - `protocol`: 协议标签
    /// - `headers`: 请求头
    /// - `body`: 请求体
    ///
    /// 返回:
    /// - 可选记录器
    fn start_http_debug(
        &self,
        method: &str,
        url: &str,
        protocol: &str,
        headers: &[(String, String)],
        body: &Value,
    ) -> Option<HttpDebugRecorder> {
        let config = self.http_debug.as_ref()?;
        match HttpDebugRecorder::start(
            config,
            method,
            url,
            &self.provider.id,
            protocol,
            headers,
            body,
        ) {
            Ok(recorder) => recorder,
            Err(err) => {
                eprintln!("[sai] HTTP debug start failed: {err:#}");
                None
            }
        }
    }

    pub async fn chat_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
        mut on_chunk: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(ChatStreamChunk) -> Result<()>,
    {
        self.chat_stream_events(messages, tools, |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                on_chunk(chunk)?;
            }
            Ok(())
        })
        .await
    }

    /// 发送流式对话并透出内部流式事件。
    ///
    /// 参数:
    /// - `messages`: 聊天消息列表
    /// - `tools`: 当前可用工具定义
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 聊天结果
    pub async fn chat_stream_events<F>(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
        mut on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(ChatStreamEvent) -> Result<()>,
    {
        let protocol = ProviderProtocol::from_provider(&self.provider)?;
        if protocol == ProviderProtocol::Anthropic
            || (protocol == ProviderProtocol::Auto
                && provider_looks_official_anthropic(&self.provider))
        {
            return self
                .chat_anthropic_stream(messages, tools, &mut on_event)
                .await;
        }
        if protocol == ProviderProtocol::OpenAiResponses
            || (protocol == ProviderProtocol::Auto && self.uses_openai_responses())
        {
            if let Some(result) = self
                .chat_responses_stream(messages.clone(), tools.clone(), &mut on_event)
                .await?
            {
                return Ok(result);
            }
            if protocol == ProviderProtocol::OpenAiResponses {
                bail!("OpenAI Responses protocol is not supported by this provider");
            }
        }
        let request = ChatRequest {
            model: self.provider.default_model.clone(),
            messages,
            temperature: self.provider.temperature,
            stream: true,
            max_tokens: self
                .provider
                .model_max_output_tokens_for(&self.provider.default_model),
            tools: (!tools.is_empty()).then_some(tools),
            chat_template_kwargs: taotoken_glm_chat_template_kwargs(&self.provider),
        };
        let request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::OpenAiChat,
        )?;
        let url = format!(
            "{}/chat/completions",
            self.provider.base_url.trim_end_matches('/')
        );
        let user_agent = resolve_provider_user_agent(&self.provider);
        let mut base_headers = bearer_request_headers(&self.api_key, &[]);
        base_headers.push(("User-Agent".to_string(), user_agent.clone()));
        let headers = merge_provider_extra_headers(base_headers, &self.provider);
        let mut debug = self.start_http_debug("POST", &url, "openai-chat", &headers, &request);
        let response = with_provider_extra_headers(
            apply_provider_user_agent(
                self.client
                    .post(&url)
                    .bearer_auth(&self.api_key)
                    .json(&request),
                &self.provider,
            ),
            &self.provider,
        )
        .send()
        .await?;
        let status = response.status();
        if let Some(debug) = debug.as_ref() {
            let _ = debug.write_response_headers(status.as_u16(), response.headers());
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if let Some(debug) = debug.as_ref() {
                let _ = debug.finish_error(status.as_u16(), &body);
            }
            bail!(
                "{} ({status}): {body}",
                t("chat completions stream request failed", "聊天流式请求失败",)
            );
        }

        // 按字节缓冲再按行解码，避免多字节 UTF-8 被 chunk 切断后变成 U+FFFD
        let mut buffer = Utf8LineBuffer::default();
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut tool_calls = ToolCallAccumulator::default();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for line in buffer.push(&chunk)? {
                if let Some(debug) = debug.as_mut() {
                    debug.append_stream_line(&line);
                }
                if let Some(done) = handle_sse_line(
                    &line,
                    &mut content,
                    &mut content_emitted,
                    &mut reasoning,
                    &mut reasoning_emitted,
                    &mut usage,
                    &mut tool_calls,
                    &mut on_event,
                )? {
                    if done {
                        let result =
                            finalize_stream_result(content, reasoning, usage, tool_calls.finish())?;
                        if let Some(debug) = debug.as_ref() {
                            let _ = debug.finish_ok(&result);
                        }
                        return Ok(result);
                    }
                }
            }
        }
        for line in buffer.finish()? {
            if let Some(debug) = debug.as_mut() {
                debug.append_stream_line(&line);
            }
            let _ = handle_sse_line(
                &line,
                &mut content,
                &mut content_emitted,
                &mut reasoning,
                &mut reasoning_emitted,
                &mut usage,
                &mut tool_calls,
                &mut on_event,
            )?;
        }
        let result = finalize_stream_result(content, reasoning, usage, tool_calls.finish())?;
        if let Some(debug) = debug.as_ref() {
            let _ = debug.finish_ok(&result);
        }
        Ok(result)
    }

    async fn chat_anthropic_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
        on_event: &mut F,
    ) -> Result<ChatResult>
    where
        F: FnMut(ChatStreamEvent) -> Result<()>,
    {
        let tools = prepare_anthropic_tools(&self.provider, tools);
        let request = AnthropicRequest {
            model: self.provider.default_model.clone(),
            system: lower_anthropic_system(&messages),
            messages: lower_anthropic_messages(messages),
            tools: (!tools.is_empty()).then(|| lower_anthropic_tools(tools)),
            stream: true,
            max_tokens: self
                .provider
                .model_max_output_tokens_for(&self.provider.default_model)
                .unwrap_or(self.provider.anthropic_max_tokens),
            temperature: Some(self.provider.temperature),
        };
        let request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::Anthropic,
        )?;
        let url = format!("{}/messages", self.provider.base_url.trim_end_matches('/'));
        let user_agent = resolve_provider_user_agent(&self.provider);
        let mut base_headers = anthropic_request_headers(&self.api_key);
        base_headers.push(("User-Agent".to_string(), user_agent));
        let headers = merge_provider_extra_headers(base_headers, &self.provider);
        let mut debug = self.start_http_debug("POST", &url, "anthropic", &headers, &request);
        // 【Anthropic】【Messages 请求】1. 首先使用当前 thinking 配置发送请求
        let response = self.send_anthropic_request(&url, &request).await?;
        let status = response.status();
        if let Some(debug) = debug.as_ref() {
            let _ = debug.write_response_headers(status.as_u16(), response.headers());
        }
        let response = if status.is_success() {
            response
        } else {
            let body = response.text().await.unwrap_or_default();
            if let Some(debug) = debug.as_ref() {
                let _ = debug.finish_error(status.as_u16(), &body);
            }
            // 【Anthropic】【Thinking 降级】2. 仅在服务端明确不支持 thinking 时移除参数重试一次
            if request.get("thinking").is_some()
                && anthropic_thinking_unsupported(status.as_u16(), &body)
            {
                let mut fallback_request = request.clone();
                if let Some(object) = fallback_request.as_object_mut() {
                    object.remove("thinking");
                }
                debug = self.start_http_debug(
                    "POST",
                    &url,
                    "anthropic-thinking-fallback",
                    &headers,
                    &fallback_request,
                );
                let fallback_response =
                    self.send_anthropic_request(&url, &fallback_request).await?;
                let fallback_status = fallback_response.status();
                if let Some(debug) = debug.as_ref() {
                    let _ = debug.write_response_headers(
                        fallback_status.as_u16(),
                        fallback_response.headers(),
                    );
                }
                if fallback_status.is_success() {
                    // 【Anthropic】【Thinking 降级】3. 降级成功后继续消费 Messages 流
                    fallback_response
                } else {
                    let fallback_body = fallback_response.text().await.unwrap_or_default();
                    if let Some(debug) = debug.as_ref() {
                        let _ = debug.finish_error(fallback_status.as_u16(), &fallback_body);
                    }
                    let hint = claude_protocol_hint(&self.provider);
                    bail!(
                        "{} ({fallback_status}): {fallback_body}{hint}",
                        t(
                            "anthropic messages stream request failed",
                            "Anthropic Messages 流式请求失败"
                        )
                    );
                }
            } else {
                let hint = claude_protocol_hint(&self.provider);
                bail!(
                    "{} ({status}): {body}{hint}",
                    t(
                        "anthropic messages stream request failed",
                        "Anthropic Messages 流式请求失败"
                    )
                );
            }
        };

        let mut state = AnthropicStreamState::default();
        let mut buffer = SseDataBuffer::default();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            for data in buffer.push(&chunk)? {
                if let Some(debug) = debug.as_mut() {
                    // Anthropic 聚合后的 data 载荷，写成 SSE data 行便于回放
                    debug.append_stream_line(&format!("data: {data}"));
                    debug.append_stream_line("");
                }
                if handle_anthropic_sse_data(&data, &mut state, &mut *on_event)? {
                    let result = finalize_stream_result(
                        state.content,
                        state.reasoning,
                        state.usage,
                        state.tool_calls.finish(),
                    )?;
                    if let Some(debug) = debug.as_ref() {
                        let _ = debug.finish_ok(&result);
                    }
                    return Ok(result);
                }
            }
        }
        for data in buffer.finish()? {
            if let Some(debug) = debug.as_mut() {
                debug.append_stream_line(&format!("data: {data}"));
                debug.append_stream_line("");
            }
            let _ = handle_anthropic_sse_data(&data, &mut state, &mut *on_event)?;
        }
        let result = finalize_stream_result(
            state.content,
            state.reasoning,
            state.usage,
            state.tool_calls.finish(),
        )?;
        if let Some(debug) = debug.as_ref() {
            let _ = debug.finish_ok(&result);
        }
        Ok(result)
    }

    /// 发送一次 Anthropic Messages 请求。
    ///
    /// 参数:
    /// - `url`: Messages API 地址
    /// - `request`: 已应用思考与自定义字段的请求体
    ///
    /// 返回:
    /// - HTTP 响应
    async fn send_anthropic_request(
        &self,
        url: &str,
        request: &Value,
    ) -> Result<reqwest::Response> {
        let builder = apply_provider_user_agent(
            self.client
                .post(url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(request),
            &self.provider,
        );
        Ok(with_provider_extra_headers(builder, &self.provider)
            .send()
            .await?)
    }

    async fn chat_responses_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<ToolDefinition>,
        on_event: &mut F,
    ) -> Result<Option<ChatResult>>
    where
        F: FnMut(ChatStreamEvent) -> Result<()>,
    {
        // 1. 拆出 system 作为 instructions；其余消息进 input
        let (instructions, input_messages) = split_responses_instructions(messages);
        let codex = prefers_codex_responses_shape(
            &self.provider.default_model,
            &self.provider.base_url,
            &self.provider.client_style,
        );
        let session_key = uuid::Uuid::new_v4().to_string();
        let request = ResponsesRequest {
            model: self.provider.default_model.clone(),
            input: lower_responses_messages(input_messages),
            instructions: Some(instructions.unwrap_or_default()),
            stream: true,
            store: false,
            tool_choice: "auto".to_string(),
            parallel_tool_calls: true,
            include: vec!["reasoning.encrypted_content".to_string()],
            // Codex 通道会剥离 max_output_tokens / temperature；非 Codex 仍可带
            max_output_tokens: if codex {
                None
            } else {
                self.provider
                    .model_max_output_tokens_for(&self.provider.default_model)
            },
            tools: (!tools.is_empty()).then(|| lower_responses_tools(tools)),
            reasoning: Some(ResponsesReasoning {
                effort: Some(if codex { "low" } else { "medium" }),
                summary: Some(if codex { "auto" } else { "concise" }),
            }),
            temperature: if codex {
                None
            } else {
                Some(self.provider.temperature)
            },
            prompt_cache_key: codex.then(|| session_key.clone()),
            client_metadata: codex.then(|| {
                serde_json::json!({ "session_id": session_key })
            }),
        };
        let request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::OpenAiResponses,
        )?;
        let url = format!("{}/responses", self.provider.base_url.trim_end_matches('/'));
        let user_agent = resolve_provider_user_agent(&self.provider);
        let headers = merge_provider_extra_headers(
            if codex {
                codex_responses_request_headers(&self.api_key, &session_key, &user_agent)
            } else {
                let mut headers = bearer_request_headers(&self.api_key, &[]);
                headers.push(("User-Agent".to_string(), user_agent));
                headers
            },
            &self.provider,
        );
        let mut debug = self.start_http_debug("POST", &url, "openai-responses", &headers, &request);
        let mut req = self.client.post(&url).bearer_auth(&self.api_key).json(&request);
        if codex {
            // 额外 Codex 请求头（Authorization / Content-Type 由 bearer + json 处理）
            req = apply_codex_response_headers(req, &self.provider, &session_key);
        } else {
            req = apply_provider_user_agent(req, &self.provider);
        }
        let response = with_provider_extra_headers(req, &self.provider).send().await?;
        let status = response.status();
        if let Some(debug) = debug.as_ref() {
            let _ = debug.write_response_headers(status.as_u16(), response.headers());
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if responses_unsupported(status.as_u16(), &body) {
                return Ok(None);
            }
            if let Some(debug) = debug.as_ref() {
                let _ = debug.finish_error(status.as_u16(), &body);
            }
            bail!(
                "{} ({status}): {body}",
                t("responses stream request failed", "Responses 流式请求失败")
            );
        }

        let mut buffer = Utf8LineBuffer::default();
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut content_started = false;
        let mut tool_calls = ResponsesToolAccumulator::default();
        let mut stream = response.bytes_stream();
        // Codex / 部分网关在正文结束后可能不发 response.completed 且保持连接；
        // 正文已开始后，空闲超过阈值则按已收到内容收尾，避免前端永久 thinking。
        let idle_limit = responses_stream_idle_timeout(self.provider.timeout_seconds);
        loop {
            let next = tokio::time::timeout(idle_limit, stream.next()).await;
            let chunk = match next {
                Ok(Some(chunk)) => chunk?,
                Ok(None) => break,
                Err(_) if content_started || !content.is_empty() || !reasoning.is_empty() => {
                    // 已有输出且长时间无新字节：按成功结束处理
                    break;
                }
                Err(_) => {
                    bail!(
                        "{}",
                        t(
                            "responses stream idle timeout before any output",
                            "Responses 流在输出前空闲超时"
                        )
                    );
                }
            };
            for line in buffer.push(&chunk)? {
                if let Some(debug) = debug.as_mut() {
                    debug.append_stream_line(&line);
                }
                if handle_responses_sse_line(
                    &line,
                    &mut content,
                    &mut content_emitted,
                    &mut reasoning,
                    &mut reasoning_emitted,
                    &mut usage,
                    &mut content_started,
                    &mut tool_calls,
                    &mut *on_event,
                )? {
                    let result =
                        finalize_stream_result(content, reasoning, usage, tool_calls.finish())?;
                    if let Some(debug) = debug.as_ref() {
                        let _ = debug.finish_ok(&result);
                    }
                    return Ok(Some(result));
                }
            }
        }
        for line in buffer.finish()? {
            if let Some(debug) = debug.as_mut() {
                debug.append_stream_line(&line);
            }
            let _ = handle_responses_sse_line(
                &line,
                &mut content,
                &mut content_emitted,
                &mut reasoning,
                &mut reasoning_emitted,
                &mut usage,
                &mut content_started,
                &mut tool_calls,
                &mut *on_event,
            )?;
        }
        // 确保缓冲中的正文/推理全部推给上层，再组装结果
        flush_responses_buffers(
            &content,
            &mut content_emitted,
            &reasoning,
            &mut reasoning_emitted,
            &mut *on_event,
        )?;
        let result = finalize_stream_result(content, reasoning, usage, tool_calls.finish())?;
        if let Some(debug) = debug.as_ref() {
            let _ = debug.finish_ok(&result);
        }
        Ok(Some(result))
    }

    fn uses_openai_responses(&self) -> bool {
        let model = self.provider.default_model.to_ascii_lowercase();
        model.starts_with("gpt-5")
            || model.contains("codex")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4")
            || prefers_codex_responses_shape(
                &self.provider.default_model,
                &self.provider.base_url,
                &self.provider.client_style,
            )
    }
}


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
fn split_responses_instructions(
    messages: Vec<ChatMessage>,
) -> (Option<String>, Vec<ChatMessage>) {
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
    let joined = instructions.join("

");
    (
        (!joined.trim().is_empty()).then_some(joined),
        rest,
    )
}

/// Codex CLI 默认 User-Agent。
const CODEX_CLI_USER_AGENT: &str = "codex_cli_rs/0.144.0";
/// 非 Codex 默认 User-Agent。
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
/// - 自定义 UA；否则 Codex 风格用 Codex CLI UA，其它用 sai 默认 UA
fn resolve_provider_user_agent(provider: &ProviderConfig) -> String {
    let custom = provider.user_agent.trim();
    if !custom.is_empty() {
        return custom.to_string();
    }
    if provider.client_style.trim().eq_ignore_ascii_case("codex") {
        return CODEX_CLI_USER_AGENT.to_string();
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
        return "\nHint: official Anthropic Claude requires protocol=anthropic and base_url=https://api.anthropic.com/v1; OpenAI-compatible Claude proxies should keep openai-chat or auto.";
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
    if !matches!(status, 400 | 422) {
        return false;
    }
    let body = body.to_ascii_lowercase();
    body.contains("thinking")
        && [
            "unsupported",
            "not supported",
            "unknown",
            "invalid",
            "unrecognized",
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
