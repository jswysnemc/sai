use super::estimate::estimate_message_tokens;
use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use std::collections::{HashMap, HashSet};

/// 【上下文性能】【消息估算】只保留当前投影的文本指纹与计数，不复制历史正文。
#[derive(Default)]
pub(crate) struct MessageTokenCache {
    entries: HashMap<blake3::Hash, usize>,
}

impl MessageTokenCache {
    /// 【上下文性能】【消息估算】复用未变化消息的 token 数；参数为消息列表，返回总占用。
    pub(crate) fn count(&mut self, messages: &[ChatMessage]) -> usize {
        self.count_with(messages, estimate_message_tokens)
    }

    /// 【上下文性能】【估算刷新】重算变化内容并移除离开投影的缓存；参数为消息与估算器。
    /// @returns 当前消息总占用
    fn count_with(
        &mut self,
        messages: &[ChatMessage],
        mut estimate: impl FnMut(&ChatMessage) -> usize,
    ) -> usize {
        let mut active = HashSet::with_capacity(messages.len());
        let mut tokens = 0usize;
        for message in messages {
            let key = fingerprint(message);
            active.insert(key);
            tokens = tokens
                .saturating_add(*self.entries.entry(key).or_insert_with(|| estimate(message)));
        }
        self.entries.retain(|key, _| active.contains(key));
        tokens
    }
}

/// 【上下文性能】【内容指纹】按分词器使用的字段构造指纹，不分配整包 JSON。
/// @param message 为待估算消息；返回内容指纹
fn fingerprint(message: &ChatMessage) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    hash_text(&mut hash, &message.role);
    match &message.content {
        Some(ChatContent::Text(text)) => {
            hash.update(&[0]);
            hash_text(&mut hash, text);
        }
        Some(ChatContent::Parts(parts)) => {
            hash.update(&[1]);
            hash.update(&parts.len().to_le_bytes());
            for part in parts {
                match part {
                    ChatContentPart::Text { text } => {
                        hash.update(&[0]);
                        hash_text(&mut hash, text);
                    }
                    ChatContentPart::ImageUrl { image_url } => {
                        hash.update(&[1]);
                        hash_text(&mut hash, &image_url.url);
                    }
                }
            }
        }
        None => {
            hash.update(&[2]);
        }
    }
    hash_text(
        &mut hash,
        message.reasoning_content.as_deref().unwrap_or_default(),
    );
    for call in message.tool_calls.iter().flatten() {
        hash_text(&mut hash, &call.function.name);
        hash_text(&mut hash, &call.function.arguments);
    }
    hash.finalize()
}

/// 【上下文性能】【内容指纹】将长度和文本写入哈希，避免拼接边界碰撞。
/// @param hash 为指纹器；text 为字段；无返回值
fn hash_text(hash: &mut blake3::Hasher, text: &str) {
    hash.update(&text.len().to_le_bytes());
    hash.update(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【上下文性能】【缓存回归】中间工具结果变化时只重算该消息，并及时清理旧投影。
    /// @returns 无；不接收参数
    #[test]
    fn reuses_stable_history_and_recounts_changed_messages() {
        let mut cache = MessageTokenCache::default();
        let mut messages = vec![
            ChatMessage::plain("user", "stable"),
            ChatMessage::tool("call", "old result"),
        ];
        let mut calls = 0;
        cache.count_with(&messages, |_| {
            calls += 1;
            10
        });
        assert_eq!(calls, 2);
        cache.count_with(&messages, |_| {
            calls += 1;
            10
        });
        assert_eq!(calls, 2);
        messages[1] = ChatMessage::tool("call", "changed result");
        cache.count_with(&messages, |_| {
            calls += 1;
            10
        });
        assert_eq!(calls, 3);
        assert_eq!(cache.entries.len(), 2);
        cache.count(&[]);
        assert!(cache.entries.is_empty());
    }

    /// 【上下文性能】【估算一致性】正文、图片、思考变化后结果与完整估算一致。
    /// @returns 无；不接收参数
    #[test]
    fn matches_full_estimates_for_text_and_images() {
        let mut cache = MessageTokenCache::default();
        let mut messages = vec![
            ChatMessage::plain("user", "你好 world"),
            ChatMessage::user_with_image("image", "https://example.com/a.png"),
        ];
        for step in 0..4 {
            messages[0] = ChatMessage::plain("assistant", format!("answer {step}"))
                .with_reasoning(Some(format!("reason {step}")));
            assert_eq!(
                cache.count(&messages),
                super::super::estimate_chat_messages_tokens(&messages)
            );
        }
    }
}
