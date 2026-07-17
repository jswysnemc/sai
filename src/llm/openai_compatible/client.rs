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
            "anthropic"
            | "anthropic-messages"
            | "messages"
            | "claude"
            | "claude-code"
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
            || (protocol == ProviderProtocol::Auto && provider_looks_official_anthropic(&self.provider))
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
        let headers = bearer_request_headers(&self.api_key, &[]);
        let mut debug =
            self.start_http_debug("POST", &url, "openai-chat", &headers, &request);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request)
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
                        let result = finalize_stream_result(
                            content,
                            reasoning,
                            usage,
                            tool_calls.finish(),
                        )?;
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
            max_tokens: self.provider.anthropic_max_tokens,
            temperature: Some(self.provider.temperature),
        };
        let request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::Anthropic,
        )?;
        let url = format!("{}/messages", self.provider.base_url.trim_end_matches('/'));
        let headers = anthropic_request_headers(&self.api_key);
        let mut debug =
            self.start_http_debug("POST", &url, "anthropic", &headers, &request);
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
                let fallback_response = self.send_anthropic_request(&url, &fallback_request).await?;
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
    async fn send_anthropic_request(&self, url: &str, request: &Value) -> Result<reqwest::Response> {
        Ok(self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(request)
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
        let request = ResponsesRequest {
            model: self.provider.default_model.clone(),
            input: lower_responses_messages(messages),
            instructions: None,
            stream: true,
            tools: (!tools.is_empty()).then(|| lower_responses_tools(tools)),
            reasoning: Some(ResponsesReasoning {
                effort: Some("medium"),
                summary: Some("concise"),
            }),
            temperature: Some(self.provider.temperature),
        };
        let request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::OpenAiResponses,
        )?;
        let url = format!("{}/responses", self.provider.base_url.trim_end_matches('/'));
        let headers = bearer_request_headers(&self.api_key, &[]);
        let mut debug =
            self.start_http_debug("POST", &url, "openai-responses", &headers, &request);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;
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
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
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
        let result = finalize_stream_result(content, reasoning, usage, tool_calls.finish())?;
        if let Some(debug) = debug.as_ref() {
            let _ = debug.finish_ok(&result);
        }
        Ok(Some(result))
    }

    fn uses_openai_responses(&self) -> bool {
        let model = self.provider.default_model.to_ascii_lowercase();
        model.starts_with("gpt-5")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4")
    }
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
        || provider.display_name.to_ascii_lowercase().contains("claude");
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
        && ["unsupported", "not supported", "unknown", "invalid", "unrecognized"]
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
        Some(WEB_SEARCH_TOOL_MODE_HIDE) => tools
            .into_iter()
            .filter(|tool| tool.function.name != "web_search")
            .collect(),
        Some(WEB_SEARCH_TOOL_MODE_RENAME) => tools
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
