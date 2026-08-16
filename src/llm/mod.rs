mod http_debug;
mod openai_compatible;
mod stream_event;
mod thinking;
mod tool_call_stream;
mod transport_retry;

pub use http_debug::RequestContextGuard as HttpDebugRequestContextGuard;
pub use http_debug::SessionGuard as HttpDebugSessionGuard;
pub use openai_compatible::OpenAiCompatibleClient;
pub use stream_event::{ChatStreamEvent, ToolCallStreamProgress};
pub(crate) use transport_retry::{error_detail_text, is_transient_transport_error};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 上一轮的思考内容；仅在供应商要求回传历史思考时序列化。
    ///
    /// Moonshot 的 Preserved Thinking 要求多轮请求原样带回历史
    /// `reasoning_content`，否则 kimi-k2.7-code 一类模型会报错。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Parts(Vec<ChatContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlContent },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlContent {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(ChatContent::Text(content.into())),
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Option<Vec<ToolCall>>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(ChatContent::Text(content.into())),
            tool_call_id: None,
            tool_calls,
            reasoning_content: None,
        }
    }

    /// 附加上一轮的思考内容。
    ///
    /// 参数:
    /// - `reasoning`: 历史思考文本；空值表示该轮没有思考
    ///
    /// 返回:
    /// - 已附加思考内容的消息
    pub fn with_reasoning(mut self, reasoning: Option<String>) -> Self {
        self.reasoning_content = reasoning.filter(|text| !text.trim().is_empty());
        self
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(ChatContent::Text(content.into())),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn plain(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(ChatContent::Text(content.into())),
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn user_with_image(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        Self::user_with_images(text, [image_url.into()])
    }

    /// 创建包含一张或多张图片的用户消息。
    ///
    /// 参数:
    /// - `text`: 用户文本
    /// - `image_urls`: 图片 data URL 列表
    ///
    /// 返回:
    /// - 多模态用户消息
    pub fn user_with_images(
        text: impl Into<String>,
        image_urls: impl IntoIterator<Item = String>,
    ) -> Self {
        let mut parts = vec![ChatContentPart::Text { text: text.into() }];
        parts.extend(image_urls.into_iter().map(|url| ChatContentPart::ImageUrl {
            image_url: ImageUrlContent { url },
        }));
        Self {
            role: "user".to_string(),
            content: Some(ChatContent::Parts(parts)),
            tool_call_id: None,
            tool_calls: None,
            reasoning_content: None,
        }
    }
}

/// 单次模型调用的令牌用量。
///
/// `prompt_tokens` 始终是输入侧总量（含命中缓存的部分），上下文预算判断依赖该口径。
/// 缓存明细单独记录，因为供应商对缓存读写另有计价系数：
/// 命中读取通常按 10% 计价，写入缓存通常按 125% 计价，
/// 只看总量会与账单出现数量级差异。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    /// `prompt_tokens` 中命中缓存读取的部分
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// `prompt_tokens` 中写入缓存的部分
    #[serde(default)]
    pub cache_write_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub content: String,
    pub reasoning: Option<String>,
    pub usage: Option<Usage>,
    pub tool_calls: Vec<ToolCall>,
    /// 本轮从首次思考/正文输出到结束的耗时（毫秒）
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStreamKind {
    Content,
    Reasoning,
}

#[derive(Debug, Clone)]
pub struct ChatStreamChunk {
    pub kind: ChatStreamKind,
    pub text: String,
}
