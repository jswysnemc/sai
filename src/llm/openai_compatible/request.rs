use sha2::{Digest, Sha256};

const RESPONSES_CALL_ID_MAX_CHARS: usize = 64;

fn responses_unsupported(status: u16, body: &str) -> bool {
    if status == 404 || status == 405 {
        return true;
    }
    if status != 400 {
        return false;
    }
    let body = body.to_ascii_lowercase();
    body.contains("unsupported")
        || body.contains("not supported")
        || body.contains("unknown parameter")
        || body.contains("invalid endpoint")
        || body.contains("not found")
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<ChatTemplateKwargs>,
}

#[derive(Debug, Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ResponsesReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct ResponsesReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    stream: bool,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: AnthropicImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum AnthropicImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

fn taotoken_glm_chat_template_kwargs(provider: &ProviderConfig) -> Option<ChatTemplateKwargs> {
    let base_url = provider.base_url.to_ascii_lowercase();
    let model = provider.default_model.to_ascii_lowercase();
    if base_url.contains("taotoken.net") && model.starts_with("glm") {
        Some(ChatTemplateKwargs {
            enable_thinking: true,
        })
    } else {
        None
    }
}

fn lower_responses_messages(messages: Vec<ChatMessage>) -> Vec<Value> {
    messages
        .into_iter()
        .flat_map(|message| match message.role.as_str() {
            "system" => vec![json!({"role": "system", "content": chat_content_text(message.content)})],
            "user" => vec![json!({"role": "user", "content": lower_responses_user_content(message.content)})],
            "assistant" => lower_responses_assistant_message(message),
            "tool" => vec![json!({"type": "function_call_output", "call_id": responses_call_id(&message.tool_call_id.unwrap_or_default()), "output": chat_content_text(message.content)})],
            role => vec![json!({"role": role, "content": chat_content_text(message.content)})],
        })
        .collect()
}

fn lower_responses_assistant_message(message: ChatMessage) -> Vec<Value> {
    let mut items = Vec::new();
    let text = chat_content_text(message.content);
    if !text.trim().is_empty() {
        items
            .push(json!({"role": "assistant", "content": [{"type": "output_text", "text": text}]}));
    }
    if let Some(tool_calls) = message.tool_calls {
        items.extend(tool_calls.into_iter().map(|call| {
            json!({
                "type": "function_call",
                "call_id": responses_call_id(&call.id),
                "name": call.function.name,
                "arguments": call.function.arguments,
            })
        }));
    }
    items
}

/// 将工具调用标识归一化到 OpenAI Responses 的 64 字符限制内。
///
/// 参数:
/// - `value`: 原始 provider 工具调用标识
///
/// 返回:
/// - 可稳定配对的 Responses 工具调用标识
fn responses_call_id(value: &str) -> String {
    if value.chars().count() <= RESPONSES_CALL_ID_MAX_CHARS {
        return value.to_string();
    }
    let digest = hex::encode(Sha256::digest(value.as_bytes()));
    format!("c{}", &digest[..RESPONSES_CALL_ID_MAX_CHARS - 1])
}

fn lower_responses_user_content(content: Option<super::ChatContent>) -> Vec<Value> {
    match content {
        Some(super::ChatContent::Parts(parts)) => parts
            .into_iter()
            .map(|part| match part {
                super::ChatContentPart::Text { text } => {
                    json!({"type": "input_text", "text": text})
                }
                super::ChatContentPart::ImageUrl { image_url } => {
                    json!({"type": "input_image", "image_url": image_url.url})
                }
            })
            .collect(),
        Some(super::ChatContent::Text(text)) => vec![json!({"type": "input_text", "text": text})],
        None => vec![json!({"type": "input_text", "text": ""})],
    }
}

fn chat_content_text(content: Option<super::ChatContent>) -> String {
    match content {
        Some(super::ChatContent::Text(text)) => text,
        Some(super::ChatContent::Parts(parts)) => parts
            .into_iter()
            .filter_map(|part| match part {
                super::ChatContentPart::Text { text } => Some(text),
                super::ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

fn lower_responses_tools(tools: Vec<ToolDefinition>) -> Vec<Value> {
    tools
        .into_iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.function.name,
                "description": tool.function.description,
                "parameters": openai_tool_input_schema(tool.function.parameters),
                "strict": false,
            })
        })
        .collect()
}

fn lower_anthropic_system(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .take_while(|message| message.role == "system")
        .map(|message| chat_content_text_ref(message.content.as_ref()))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
        .into_non_empty()
}

fn lower_anthropic_messages(messages: Vec<ChatMessage>) -> Vec<AnthropicMessage> {
    let mut output = Vec::new();
    let mut skipped_initial_system = true;
    for message in messages {
        if skipped_initial_system && message.role == "system" {
            continue;
        }
        skipped_initial_system = false;
        match message.role.as_str() {
            "user" => output.push(AnthropicMessage {
                role: "user".to_string(),
                content: lower_anthropic_user_content(message.content),
            }),
            "assistant" => output.push(AnthropicMessage {
                role: "assistant".to_string(),
                content: lower_anthropic_assistant_content(message),
            }),
            "tool" => output.push(AnthropicMessage {
                role: "user".to_string(),
                content: vec![AnthropicContentBlock::ToolResult {
                    tool_use_id: message.tool_call_id.unwrap_or_default(),
                    content: chat_content_text(message.content),
                }],
            }),
            "system" => output.push(AnthropicMessage {
                role: "user".to_string(),
                content: vec![AnthropicContentBlock::Text {
                    text: wrap_system_update(chat_content_text(message.content)),
                }],
            }),
            _ => output.push(AnthropicMessage {
                role: "user".to_string(),
                content: vec![AnthropicContentBlock::Text {
                    text: chat_content_text(message.content),
                }],
            }),
        }
    }
    output
}

fn lower_anthropic_user_content(content: Option<super::ChatContent>) -> Vec<AnthropicContentBlock> {
    match content {
        Some(super::ChatContent::Parts(parts)) => parts
            .into_iter()
            .filter_map(|part| match part {
                super::ChatContentPart::Text { text } => Some(AnthropicContentBlock::Text { text }),
                super::ChatContentPart::ImageUrl { image_url } => {
                    lower_anthropic_image_url(&image_url.url)
                }
            })
            .collect(),
        Some(super::ChatContent::Text(text)) => vec![AnthropicContentBlock::Text { text }],
        None => vec![AnthropicContentBlock::Text {
            text: String::new(),
        }],
    }
}

fn lower_anthropic_image_url(url: &str) -> Option<AnthropicContentBlock> {
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(AnthropicContentBlock::Image {
            source: AnthropicImageSource::Url {
                url: url.to_string(),
            },
        });
    }
    let data = url.strip_prefix("data:")?;
    let (media_type, base64) = data.split_once(";base64,")?;
    Some(AnthropicContentBlock::Image {
        source: AnthropicImageSource::Base64 {
            media_type: media_type.to_string(),
            data: base64.to_string(),
        },
    })
}

fn lower_anthropic_assistant_content(message: ChatMessage) -> Vec<AnthropicContentBlock> {
    let mut content = Vec::new();
    let text = chat_content_text(message.content);
    if !text.trim().is_empty() {
        content.push(AnthropicContentBlock::Text { text });
    }
    if let Some(tool_calls) = message.tool_calls {
        content.extend(
            tool_calls
                .into_iter()
                .map(|call| AnthropicContentBlock::ToolUse {
                    id: call.id,
                    name: call.function.name,
                    input: serde_json::from_str(&call.function.arguments)
                        .unwrap_or_else(|_| json!({})),
                }),
        );
    }
    if content.is_empty() {
        content.push(AnthropicContentBlock::Text {
            text: String::new(),
        });
    }
    content
}

fn lower_anthropic_tools(tools: Vec<ToolDefinition>) -> Vec<AnthropicTool> {
    tools
        .into_iter()
        .map(|tool| AnthropicTool {
            name: tool.function.name,
            description: tool.function.description,
            input_schema: tool.function.parameters,
        })
        .collect()
}

fn wrap_system_update(text: String) -> String {
    format!(
        "<system-update>\n{}\n</system-update>",
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}

trait IntoNonEmpty {
    fn into_non_empty(self) -> Option<String>;
}

impl IntoNonEmpty for String {
    fn into_non_empty(self) -> Option<String> {
        (!self.trim().is_empty()).then_some(self)
    }
}

fn chat_content_text_ref(content: Option<&super::ChatContent>) -> String {
    match content {
        Some(super::ChatContent::Text(text)) => text.clone(),
        Some(super::ChatContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                super::ChatContentPart::Text { text } => Some(text.clone()),
                super::ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

fn openai_tool_input_schema(schema: Value) -> Value {
    let flattened = flatten_top_level_any_of(schema);
    let normalized = remove_null_any_of(flattened);
    if normalized.is_object() {
        normalized
    } else {
        json!({"type": "object"})
    }
}

fn flatten_top_level_any_of(schema: Value) -> Value {
    let Some(object) = schema.as_object() else {
        return json!({"type": "object"});
    };
    let Some(variants) = object.get("anyOf").and_then(Value::as_array) else {
        let mut cloned = object.clone();
        cloned.insert("type".to_string(), Value::String("object".to_string()));
        return Value::Object(cloned);
    };
    let mut properties = serde_json::Map::new();
    for variant in variants.iter().filter_map(Value::as_object) {
        if let Some(variant_properties) = variant.get("properties").and_then(Value::as_object) {
            for (key, value) in variant_properties {
                properties.insert(key.clone(), value.clone());
            }
        }
    }
    let mut flattened = object
        .iter()
        .filter(|(key, _)| key.as_str() != "anyOf")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    flattened.insert("type".to_string(), Value::String("object".to_string()));
    flattened.insert("properties".to_string(), Value::Object(properties));
    flattened.insert("additionalProperties".to_string(), Value::Bool(false));
    Value::Object(flattened)
}

fn remove_null_any_of(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(remove_null_any_of).collect()),
        Value::Object(mut object) => {
            let any_of = object.remove("anyOf");
            let mut object = object
                .into_iter()
                .map(|(key, value)| (key, remove_null_any_of(value)))
                .collect::<serde_json::Map<_, _>>();
            let Some(Value::Array(variants)) = any_of else {
                return Value::Object(object);
            };
            let variants = variants
                .into_iter()
                .filter(|variant| variant.get("type").and_then(Value::as_str) != Some("null"))
                .map(remove_null_any_of)
                .collect::<Vec<_>>();
            if variants.len() == 1 {
                if let Some(variant_object) =
                    variants.first().and_then(|item| item.as_object().cloned())
                {
                    object.extend(variant_object);
                    return Value::Object(object);
                }
            }
            object.insert("anyOf".to_string(), Value::Array(variants));
            Value::Object(object)
        }
        value => value,
    }
}
