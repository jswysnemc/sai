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
                && (provider_looks_official_anthropic(&self.provider)
                    || provider_uses_claude_code_style(&self.provider)))
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
        // 【OpenAI兼容】【聊天流式】1. 先按当前 thinking 配置发送
        let mut request = request;
        let mut response = with_provider_extra_headers(
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
        let mut status = response.status();
        if let Some(debug) = debug.as_ref() {
            let _ = debug.write_response_headers(status.as_u16(), response.headers());
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            // 【OpenAI兼容】【Thinking 降级】2. 明确拒绝 thinking 时移除后重试一次
            if request.get("thinking").is_some()
                && openai_chat_thinking_unsupported(status.as_u16(), &body)
            {
                if let Some(object) = request.as_object_mut() {
                    object.remove("thinking");
                }
                if let Some(debug) = debug.as_ref() {
                    let _ = debug.finish_error(status.as_u16(), &body);
                }
                let retry_debug =
                    self.start_http_debug("POST", &url, "openai-chat-thinking-fallback", &headers, &request);
                response = with_provider_extra_headers(
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
                status = response.status();
                if let Some(debug) = retry_debug.as_ref() {
                    let _ = debug.write_response_headers(status.as_u16(), response.headers());
                }
                if !status.is_success() {
                    let retry_body = response.text().await.unwrap_or_default();
                    if let Some(debug) = retry_debug.as_ref() {
                        let _ = debug.finish_error(status.as_u16(), &retry_body);
                    }
                    bail!(
                        "{} ({status}): {retry_body}",
                        t("chat completions stream request failed", "聊天流式请求失败",)
                    );
                }
                debug = retry_debug;
            } else {
                if let Some(debug) = debug.as_ref() {
                    let _ = debug.finish_error(status.as_u16(), &body);
                }
                bail!(
                    "{} ({status}): {body}",
                    t("chat completions stream request failed", "聊天流式请求失败",)
                );
            }
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
        let claude = provider_uses_claude_code_style(&self.provider);
        let session_id = uuid::Uuid::new_v4().to_string();
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
        let mut request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::Anthropic,
        )?;
        // Claude Code 通道：system 数组 / metadata / adaptive thinking
        if claude {
            apply_claude_code_body_shape(
                &mut request,
                &session_id,
                &self.provider.thinking_level,
            );
        }
        let mut url = format!("{}/messages", self.provider.base_url.trim_end_matches('/'));
        if claude {
            url = claude_code_messages_url(&url);
        }
        let user_agent = resolve_provider_user_agent(&self.provider);
        let base_headers = if claude {
            claude_code_request_headers(
                &self.api_key,
                &session_id,
                &user_agent,
                self.provider.claude_1m_context,
            )
        } else {
            let mut headers = anthropic_request_headers(&self.api_key);
            headers.push(("User-Agent".to_string(), user_agent));
            headers
        };
        let headers = merge_provider_extra_headers(base_headers, &self.provider);
        let mut debug = self.start_http_debug("POST", &url, "anthropic", &headers, &request);
        // 【Anthropic】【Messages 请求】1. 首先使用当前 thinking 配置发送请求
        let response = self
            .send_anthropic_request(&url, &request, claude, &session_id)
            .await?;
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
                    // Claude Code 的 output_config.effort 与 thinking 成对
                    if claude {
                        if let Some(Value::Object(cfg)) = object.get_mut("output_config") {
                            cfg.remove("effort");
                            if cfg.is_empty() {
                                object.remove("output_config");
                            }
                        }
                    }
                }
                debug = self.start_http_debug(
                    "POST",
                    &url,
                    "anthropic-thinking-fallback",
                    &headers,
                    &fallback_request,
                );
                let fallback_response = self
                    .send_anthropic_request(&url, &fallback_request, claude, &session_id)
                    .await?;
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
    /// - `claude`: 是否 Claude Code 模拟
    /// - `session_id`: Claude Code 会话 UUID
    ///
    /// 返回:
    /// - HTTP 响应
    async fn send_anthropic_request(
        &self,
        url: &str,
        request: &Value,
        claude: bool,
        session_id: &str,
    ) -> Result<reqwest::Response> {
        let builder = if claude {
            // 【Claude】【Messages 请求头】1. 使用 Claude Code 通道头
            let user_agent = resolve_provider_user_agent(&self.provider);
            let req = self
                .client
                .post(url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header(
                    "anthropic-beta",
                    claude_code_beta_header(self.provider.claude_1m_context),
                )
                .header("anthropic-dangerous-direct-browser-access", "true")
                .header("User-Agent", user_agent)
                .header("x-app", "cli")
                .header("x-claude-code-session-id", session_id)
                .header("x-stainless-lang", "js")
                .header("x-stainless-package-version", "0.81.0")
                .header("x-stainless-runtime", "node")
                .header("x-stainless-runtime-version", "v24.3.0")
                .header("x-stainless-retry-count", "0")
                .json(request);
            // 2. 再合并供应商自定义头
            with_provider_extra_headers(req, &self.provider)
        } else {
            let builder = apply_provider_user_agent(
                self.client
                    .post(url)
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(request),
                &self.provider,
            );
            with_provider_extra_headers(builder, &self.provider)
        };
        Ok(builder.send().await?)
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
