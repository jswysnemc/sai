use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【模型接口】【助手原文】保存协议专有块，避免展示用文本转换丢失签名与顺序。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "protocol", content = "blocks", rename_all = "snake_case")]
pub enum ProviderAssistantContent {
    Anthropic(Vec<Value>),
}
