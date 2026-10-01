impl OpenAiCompatibleClient {
    /// 【模型接口】【Anthropic 流】发送消息并消费流式响应，保留协议降级与调试记录。
    /// @param messages 消息；tools 工具定义；on_event 流式事件回调
    /// @returns 完整响应或请求错误
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
        let tools = prepare_anthropic_tools(&self.provider, tools);
        let session_id = stable_request_cache_key(&self.provider.default_model, &messages, &tools);
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
            temperature: self.provider.temperature,
        };
        let mut request = apply_provider_body_options(
            serde_json::to_value(request)?,
            &self.provider,
            ThinkingProtocol::Anthropic,
        )?;
        // Claude Code 通道：system 数组 / metadata / adaptive thinking
        if claude {
            apply_claude_code_body_shape(&mut request, &session_id, &self.provider.thinking_level);
        }
        let mut url = format!("{}/messages", self.provider.base_url.trim_end_matches('/'));
        if claude {
            url = claude_code_messages_url(&url);
        }
        let user_agent = resolve_provider_user_agent(&self.provider);
        // 本次请求轮询到的密钥；Claude 与非 Claude 通道共用
        let api_key = self.request_api_key();
        let base_headers = if claude {
            claude_code_request_headers(
                api_key,
                &session_id,
                &user_agent,
                self.provider.claude_1m_context,
            )
        } else {
            let mut headers = anthropic_request_headers(api_key);
            headers.push(("User-Agent".to_string(), user_agent));
            headers
        };
        let headers = merge_provider_extra_headers(base_headers, &self.provider);
        let mut debug = self.start_http_debug("POST", &url, "anthropic", &headers, &request);
        // 【Anthropic】【Messages 请求】1. 首先使用当前 thinking 配置发送请求
        let response = self
            .send_anthropic_request(&url, &request, claude, &session_id, api_key)
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
                    .send_anthropic_request(&url, &fallback_request, claude, &session_id, api_key)
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
        let mut completed = false;
        for data in buffer.finish()? {
            if let Some(debug) = debug.as_mut() {
                debug.append_stream_line(&format!("data: {data}"));
                debug.append_stream_line("");
            }
            completed |= handle_anthropic_sse_data(&data, &mut state, &mut *on_event)?;
        }
        require_completion("Anthropic Messages", completed)?;
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
}
