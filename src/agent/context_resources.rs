use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const RESOURCE_STATE_OPEN: &str = "<context-resource-state>";
const RESOURCE_STATE_CLOSE: &str = "</context-resource-state>";

/// 长生命周期上下文资源的版本记录。
#[derive(Debug, Deserialize, Serialize)]
struct ResourceStateRecord {
    name: String,
    hash: String,
}

/// 仅在内容变化或压缩移除正文时生成资源更新。
///
/// 参数:
/// - `name`: 稳定资源名，例如 goal
/// - `content`: 当前完整资源正文
/// - `checkpoint_context`: 当前压缩摘要
/// - `history`: 压缩后仍可见的 provider 历史
///
/// 返回:
/// - 需要追加的资源状态；没有变化时返回空
pub(crate) fn context_resource_update(
    name: &str,
    content: &str,
    checkpoint_context: Option<&str>,
    history: &[ChatMessage],
) -> Result<Option<String>> {
    validate_name(name)?;
    let known = known_resources(checkpoint_context, history);
    if content.trim().is_empty() && !known.contains_key(name) {
        return Ok(None);
    }
    let hash = blake3::hash(content.as_bytes()).to_hex()[..16].to_string();
    if known
        .get(name)
        .is_some_and(|known_hash| known_hash == &hash)
    {
        return Ok(None);
    }
    let record = serde_json::to_string(&ResourceStateRecord {
        name: name.to_string(),
        hash: hash.clone(),
    })?;
    Ok(Some(format!(
        "{RESOURCE_STATE_OPEN}\n{record}\n{RESOURCE_STATE_CLOSE}\n<context-resource name=\"{name}\" hash=\"{hash}\">\n{}\n</context-resource>",
        content.trim()
    )))
}

/// 合并多个独立的上下文状态更新。
///
/// 参数:
/// - `updates`: 运行状态、模式与资源更新
///
/// 返回:
/// - 按固定顺序拼接后的单条 provider 用户状态
pub(crate) fn combine_context_updates(
    updates: impl IntoIterator<Item = Option<String>>,
) -> Option<String> {
    let parts = updates
        .into_iter()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// 从当前可见投影恢复已完整载入的资源版本。
///
/// 参数:
/// - `checkpoint_context`: 当前压缩摘要
/// - `history`: 当前 provider 历史
///
/// 返回:
/// - 资源名到正文哈希的映射
fn known_resources(
    checkpoint_context: Option<&str>,
    history: &[ChatMessage],
) -> BTreeMap<String, String> {
    let mut resources = BTreeMap::new();
    if let Some(context) = checkpoint_context {
        apply_text(context, &mut resources);
    }
    for message in history {
        if let Some(text) = message_text(message) {
            apply_text(&text, &mut resources);
        }
    }
    resources
}

/// 应用文本中带完整正文的资源记录。
///
/// 参数:
/// - `text`: provider 消息文本
/// - `resources`: 待更新资源映射
///
/// 返回:
/// - 无
fn apply_text(text: &str, resources: &mut BTreeMap<String, String>) {
    let mut remaining = text;
    while let Some(start) = remaining.find(RESOURCE_STATE_OPEN) {
        let after_open = &remaining[start + RESOURCE_STATE_OPEN.len()..];
        let Some(end) = after_open.find(RESOURCE_STATE_CLOSE) else {
            break;
        };
        if let Ok(record) = serde_json::from_str::<ResourceStateRecord>(after_open[..end].trim()) {
            // 只在当前状态记录与下一条状态记录之间查找正文，避免相邻资源互相污染判断
            let resource_segment = &after_open[end + RESOURCE_STATE_CLOSE.len()..];
            let next_state = resource_segment
                .find(RESOURCE_STATE_OPEN)
                .unwrap_or(resource_segment.len());
            let resource_segment = &resource_segment[..next_state];
            let open = format!(
                "<context-resource name=\"{}\" hash=\"{}\">",
                record.name, record.hash
            );
            if resource_segment.contains(&open) && resource_segment.contains("</context-resource>")
            {
                resources.insert(record.name, record.hash);
            }
        }
        remaining = &after_open[end + RESOURCE_STATE_CLOSE.len()..];
    }
}

/// 提取消息中的纯文本。
///
/// 参数:
/// - `message`: provider 消息
///
/// 返回:
/// - 不含图片的文本
fn message_text(message: &ChatMessage) -> Option<String> {
    match message.content.as_ref() {
        Some(ChatContent::Text(text)) => Some(text.clone()),
        Some(ChatContent::Parts(parts)) => Some(
            parts
                .iter()
                .filter_map(|part| match part {
                    ChatContentPart::Text { text } => Some(text.as_str()),
                    ChatContentPart::ImageUrl { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        None => None,
    }
}

/// 校验资源名可安全用于内部 XML 标签。
///
/// 参数:
/// - `name`: 资源名
///
/// 返回:
/// - 校验是否通过
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        bail!("invalid context resource name: {name}")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证相同资源只载入一次，变化后追加新版本。
    #[test]
    fn resource_updates_only_when_content_changes() {
        let first = context_resource_update("goal", "step 1", None, &[])
            .unwrap()
            .unwrap();
        let history = vec![ChatMessage::plain("user", &first)];
        assert!(context_resource_update("goal", "step 1", None, &history)
            .unwrap()
            .is_none());
        let changed = context_resource_update("goal", "step 2", None, &history)
            .unwrap()
            .unwrap();
        assert!(changed.contains("step 2"));
    }

    /// 验证压缩只保留版本记录而移除正文时会重新载入完整资源。
    #[test]
    fn resource_reloads_when_compaction_drops_body() {
        let checkpoint = r#"<context-resource-state>
{"name":"goal","hash":"deadbeef"}
</context-resource-state>"#;
        let update = context_resource_update("goal", "step 1", Some(checkpoint), &[])
            .unwrap()
            .unwrap();
        assert!(update.contains("<context-resource name=\"goal\""));
    }

    /// 验证已有资源清空时发送一次空版本，后续不重复。
    #[test]
    fn resource_clear_is_persisted_once() {
        let first = context_resource_update("goal", "step 1", None, &[])
            .unwrap()
            .unwrap();
        let history = vec![ChatMessage::plain("user", &first)];
        let cleared = context_resource_update("goal", "", None, &history)
            .unwrap()
            .unwrap();
        let cleared_history = vec![ChatMessage::plain("user", &cleared)];
        assert!(context_resource_update("goal", "", None, &cleared_history)
            .unwrap()
            .is_none());
    }

    /// 验证相邻资源中只有实际保留正文的资源才会被视为已载入。
    #[test]
    fn resource_presence_is_scoped_to_each_state_record() {
        let goal_update = context_resource_update("goal", "goal", None, &[])
            .unwrap()
            .unwrap();
        let goal_hash = blake3::hash("goal".as_bytes()).to_hex()[..16].to_string();
        let history = vec![ChatMessage::plain(
            "user",
            format!(
                "{goal_update}\n<context-resource-state>\n{{\"name\":\"meme\",\"hash\":\"meme-hash\"}}\n</context-resource-state>"
            ),
        )];

        assert!(context_resource_update("goal", "goal", None, &history)
            .unwrap()
            .is_none());
        assert!(context_resource_update("meme", "meme", None, &history)
            .unwrap()
            .is_some());
        assert!(history[0]
            .content
            .as_ref()
            .and_then(|content| match content {
                ChatContent::Text(text) => Some(text.contains(&goal_hash)),
                ChatContent::Parts(_) => None,
            })
            .unwrap_or(false));
    }
}
