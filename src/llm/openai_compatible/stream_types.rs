#[derive(Debug, Deserialize)]
struct ChatStreamResponse {
    #[serde(default, deserialize_with = "null_as_default")]
    choices: Vec<ChatStreamChoice>,
    #[serde(default, deserialize_with = "null_as_default")]
    usage: Option<ChatUsage>,
}

/// OpenAI Chat Completions 协议的用量字段。
///
/// 与领域模型 `Usage` 分开定义，避免把线格式的 `prompt_tokens_details`
/// 结构泄漏进内部数据结构。
#[derive(Debug, Deserialize)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default, deserialize_with = "null_as_default")]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

/// OpenAI 系协议里输入令牌的细分构成。
#[derive(Debug, Default, Deserialize)]
struct PromptTokensDetails {
    /// 命中提示词缓存的令牌数
    #[serde(default)]
    cached_tokens: u64,
}

impl ChatUsage {
    /// 转换成领域用量模型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 带缓存明细的用量；OpenAI 不区分缓存写入，写入量记为 0
    fn into_usage(self) -> Usage {
        let cache_read_tokens = self
            .prompt_tokens_details
            .map(|details| details.cached_tokens)
            .unwrap_or_default();
        Usage {
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            total_tokens: self.total_tokens,
            cache_read_tokens,
            cache_write_tokens: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChatStreamChoice {
    #[serde(default)]
    delta: ChatChoiceMessage,
}

#[derive(Debug, Default, Deserialize)]
struct ChatChoiceMessage {
    #[serde(default, deserialize_with = "null_as_default")]
    content: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    reasoning_content: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    reasoning: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    thinking: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    thinking_content: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    reasoning_text: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    reasoning_details: Option<serde_json::Value>,
    #[serde(default, deserialize_with = "null_as_default")]
    tool_calls: Vec<ToolCallDelta>,
}

fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Default, Deserialize)]
struct ToolCallDelta {
    index: usize,
    #[serde(default, deserialize_with = "null_as_default")]
    id: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "null_as_default")]
    kind: Option<String>,
    #[serde(default)]
    function: ToolCallFunctionDelta,
}

#[derive(Debug, Default, Deserialize)]
struct ToolCallFunctionDelta {
    #[serde(default, deserialize_with = "null_as_default")]
    name: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponsesStreamEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "null_as_default")]
    delta: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    item_id: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    item: Option<ResponsesStreamItem>,
    #[serde(default, deserialize_with = "null_as_default")]
    response: Option<ResponsesStreamResponse>,
}

#[derive(Debug, Deserialize)]
struct ResponsesStreamItem {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "null_as_default")]
    id: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    call_id: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    name: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponsesStreamResponse {
    #[serde(default, deserialize_with = "null_as_default")]
    usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponsesUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default, deserialize_with = "null_as_default")]
    input_tokens_details: Option<PromptTokensDetails>,
}

impl ResponsesUsage {
    /// 转换成领域用量模型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 带缓存明细的用量；Responses 协议同样不区分缓存写入
    fn into_usage(self) -> Usage {
        let cache_read_tokens = self
            .input_tokens_details
            .map(|details| details.cached_tokens)
            .unwrap_or_default();
        Usage {
            prompt_tokens: self.input_tokens,
            completion_tokens: self.output_tokens,
            total_tokens: self.total_tokens,
            cache_read_tokens,
            cache_write_tokens: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "null_as_default")]
    index: Option<usize>,
    #[serde(default, deserialize_with = "null_as_default")]
    message: Option<AnthropicStreamMessage>,
    #[serde(default, deserialize_with = "null_as_default")]
    content_block: Option<AnthropicStreamBlock>,
    #[serde(default, deserialize_with = "null_as_default")]
    delta: Option<AnthropicStreamDelta>,
    #[serde(default, deserialize_with = "null_as_default")]
    usage: Option<AnthropicUsage>,
    #[serde(default, deserialize_with = "null_as_default")]
    error: Option<AnthropicStreamError>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamMessage {
    #[serde(default, deserialize_with = "null_as_default")]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, deserialize_with = "null_as_default")]
    id: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    name: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    text: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    thinking: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamDelta {
    #[serde(rename = "type", default, deserialize_with = "null_as_default")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    text: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    thinking: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    partial_json: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamError {
    #[serde(rename = "type", default, deserialize_with = "null_as_default")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    message: Option<String>,
}

#[derive(Default)]
struct AnthropicStreamState {
    content: String,
    content_emitted: usize,
    reasoning: String,
    reasoning_emitted: usize,
    thinking_signature: Option<String>,
    input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    usage: Option<Usage>,
    tool_calls: AnthropicToolAccumulator,
}

#[derive(Debug, Default)]
struct AnthropicToolAccumulator {
    calls: Vec<PartialToolCall>,
    progress: ToolCallProgressTracker,
}

impl AnthropicToolAccumulator {
    fn start(
        &mut self,
        index: usize,
        block: AnthropicStreamBlock,
    ) -> Option<ToolCallStreamProgress> {
        while self.calls.len() <= index {
            self.calls.push(PartialToolCall::default());
        }
        let call = &mut self.calls[index];
        call.id = block.id.unwrap_or_else(|| format!("tool-{index}"));
        call.kind = "function".to_string();
        call.name = block.name.unwrap_or_default();
        self.progress.update(index, &call.name, &call.arguments)
    }

    fn append_arguments(&mut self, index: usize, text: String) -> Option<ToolCallStreamProgress> {
        while self.calls.len() <= index {
            self.calls.push(PartialToolCall::default());
        }
        let call = &mut self.calls[index];
        call.arguments.push_str(&text);
        self.progress.update(index, &call.name, &call.arguments)
    }

    fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .filter(|call| !call.name.trim().is_empty())
            .map(|call| ToolCall {
                id: call.id,
                kind: if call.kind.is_empty() {
                    "function".to_string()
                } else {
                    call.kind
                },
                function: ToolCallFunction {
                    name: call.name,
                    arguments: call.arguments,
                },
            })
            .collect()
    }
}

#[derive(Debug, Default)]
struct ResponsesToolAccumulator {
    calls: Vec<PartialToolCall>,
    progress: ToolCallProgressTracker,
}

impl ResponsesToolAccumulator {
    fn start(&mut self, item: ResponsesStreamItem) -> Option<ToolCallStreamProgress> {
        if item.kind != "function_call" {
            return None;
        }
        // 1. 优先 call_id，其次 item id；都为空时先占位，finish 时再生成回退 id
        let id = item
            .call_id
            .filter(|value| !value.trim().is_empty())
            .or_else(|| item.id.filter(|value| !value.trim().is_empty()))
            .unwrap_or_default();
        self.calls.push(PartialToolCall {
            id,
            kind: "function".to_string(),
            name: item.name.unwrap_or_default(),
            arguments: item.arguments.unwrap_or_default(),
        });
        let index = self.calls.len().saturating_sub(1);
        let call = &self.calls[index];
        self.progress.update(index, &call.name, &call.arguments)
    }

    fn append_arguments(
        &mut self,
        item_id: Option<String>,
        delta: String,
    ) -> Option<ToolCallStreamProgress> {
        if let Some(item_id) = item_id {
            if let Some(index) = self
                .calls
                .iter()
                .position(|call| call.id == item_id || call.id.is_empty())
            {
                let call = &mut self.calls[index];
                call.arguments.push_str(&delta);
                return self.progress.update(index, &call.name, &call.arguments);
            }
        }
        if let Some(index) = self.calls.len().checked_sub(1) {
            let call = &mut self.calls[index];
            call.arguments.push_str(&delta);
            return self.progress.update(index, &call.name, &call.arguments);
        }
        None
    }

    fn finish_item(&mut self, item: ResponsesStreamItem) -> Option<ToolCallStreamProgress> {
        if item.kind != "function_call" {
            return None;
        }
        // 1. 忽略空 id，避免覆盖已有有效 call_id
        let id = item
            .call_id
            .filter(|value| !value.trim().is_empty())
            .or_else(|| item.id.filter(|value| !value.trim().is_empty()))
            .unwrap_or_default();
        if !id.is_empty() {
            if let Some(index) = self.calls.iter().position(|call| call.id == id) {
                let call = &mut self.calls[index];
                if let Some(name) = item.name {
                    call.name = name;
                }
                if let Some(arguments) = item.arguments {
                    call.arguments = arguments;
                }
                return self.progress.update(index, &call.name, &call.arguments);
            }
            return self.start(ResponsesStreamItem {
                kind: "function_call".to_string(),
                id: None,
                call_id: Some(id),
                name: item.name,
                arguments: item.arguments,
            });
        }
        // 2. 无 id 时更新最近一条未完成调用
        if let Some(index) = self.calls.len().checked_sub(1) {
            let call = &mut self.calls[index];
            if let Some(name) = item.name {
                call.name = name;
            }
            if let Some(arguments) = item.arguments {
                call.arguments = arguments;
            }
            return self.progress.update(index, &call.name, &call.arguments);
        }
        None
    }

    fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .enumerate()
            .filter(|(_, call)| !call.name.trim().is_empty())
            .map(|(index, call)| ToolCall {
                id: if call.id.trim().is_empty() {
                    format!("call-fallback-{index}")
                } else {
                    call.id
                },
                kind: call.kind,
                function: ToolCallFunction {
                    name: call.name,
                    arguments: call.arguments,
                },
            })
            .collect()
    }
}

#[derive(Debug, Default)]
struct ToolCallAccumulator {
    calls: Vec<PartialToolCall>,
    progress: ToolCallProgressTracker,
}

#[derive(Debug, Default)]
struct PartialToolCall {
    id: String,
    kind: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn push(&mut self, delta: ToolCallDelta) -> Option<ToolCallStreamProgress> {
        while self.calls.len() <= delta.index {
            self.calls.push(PartialToolCall::default());
        }
        let call = &mut self.calls[delta.index];
        // 1. 仅接受非空 id；百炼/DashScope 后续 chunk 常带 `"id":""`，不能覆盖首个有效 call_id
        if let Some(id) = delta.id {
            if !id.trim().is_empty() {
                call.id = id;
            }
        }
        if let Some(kind) = delta.kind {
            if !kind.trim().is_empty() {
                call.kind = kind;
            }
        }
        if let Some(name) = delta.function.name {
            call.name.push_str(&name);
        }
        if let Some(arguments) = delta.function.arguments {
            call.arguments.push_str(&arguments);
        }
        self.progress
            .update(delta.index, &call.name, &call.arguments)
    }

    fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .enumerate()
            .filter(|(_, call)| !call.name.trim().is_empty())
            .map(|(index, call)| ToolCall {
                // 2. 若上游始终未给 id，按 index 生成稳定回退，避免空 call_id 导致 tool result 碰撞
                id: if call.id.trim().is_empty() {
                    format!("call-fallback-{index}")
                } else {
                    call.id
                },
                kind: if call.kind.is_empty() {
                    "function".to_string()
                } else {
                    call.kind
                },
                function: ToolCallFunction {
                    name: call.name,
                    arguments: call.arguments,
                },
            })
            .collect()
    }
}
/// SSE 行级字节缓冲：在完整换行边界再解码 UTF-8，避免 TCP/HTTP 分片切断多字节字符时
/// 被 `from_utf8_lossy` 替换成 U+FFFD（终端上的菱形问号）。
#[derive(Default)]
struct Utf8LineBuffer {
    buffer: Vec<u8>,
}

impl Utf8LineBuffer {
    /// 追加流式字节，吐出所有已完整接收的行（不含换行符）。
    ///
    /// 参数:
    /// - `bytes`: 本轮网络 chunk
    ///
    /// 返回:
    /// - 已完整的 UTF-8 行列表；末尾不完整行仍留在内部缓冲
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>> {
        // 1. 先按字节追加，不在 chunk 边界做 UTF-8 解码
        self.buffer.extend_from_slice(bytes);
        let mut lines = Vec::new();
        // 2. 仅在 `\n` 处分行；行内字节必须是合法 UTF-8
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=index).collect::<Vec<_>>();
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            lines.push(
                std::str::from_utf8(&line)
                    .context("invalid utf-8 in streaming response")?
                    .to_string(),
            );
        }
        Ok(lines)
    }

    /// 冲刷尾部残留（无换行结尾的最后一行）。
    ///
    /// 返回:
    /// - 若仅有空白则空列表；否则单行内容
    fn finish(mut self) -> Result<Vec<String>> {
        if self.buffer.iter().all(|byte| byte.is_ascii_whitespace()) {
            return Ok(Vec::new());
        }
        if self.buffer.last() == Some(&b'\r') {
            self.buffer.pop();
        }
        Ok(vec![std::str::from_utf8(&self.buffer)
            .context("invalid utf-8 in streaming response")?
            .to_string()])
    }
}

/// Anthropic 风格 SSE：在行缓冲之上聚合 `data:` 字段为事件载荷。
#[derive(Default)]
struct SseDataBuffer {
    lines: Utf8LineBuffer,
    data_lines: Vec<String>,
}

impl SseDataBuffer {
    /// 追加原始字节并返回已闭合的 SSE data 载荷。
    ///
    /// 参数:
    /// - `bytes`: 本轮网络 chunk
    ///
    /// 返回:
    /// - 完整 SSE data 事件列表（多行 data 已用 `\n` 拼接）
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>> {
        let mut events = Vec::new();
        for line in self.lines.push(bytes)? {
            if let Some(event) = self.push_line(&line) {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// 结束流时冲刷未闭合的行与 data 缓冲。
    ///
    /// 返回:
    /// - 剩余完整事件
    fn finish(mut self) -> Result<Vec<String>> {
        let mut events = Vec::new();
        for line in std::mem::take(&mut self.lines).finish()? {
            if let Some(event) = self.push_line(&line) {
                events.push(event);
            }
        }
        if !self.data_lines.is_empty() {
            events.push(self.data_lines.join("\n"));
        }
        Ok(events)
    }

    /// 处理单行 SSE 文本，空行表示事件结束。
    ///
    /// 参数:
    /// - `line`: 已解码的一行（无换行）
    ///
    /// 返回:
    /// - 事件闭合时返回 data 载荷，否则 `None`
    fn push_line(&mut self, line: &str) -> Option<String> {
        if line.is_empty() {
            if self.data_lines.is_empty() {
                return None;
            }
            return Some(std::mem::take(&mut self.data_lines).join("\n"));
        }
        if let Some(data) = line.strip_prefix("data:") {
            self.data_lines.push(data.trim_start().to_string());
        }
        None
    }
}
