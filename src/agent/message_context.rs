use crate::llm::{ChatContent, ChatContentPart, ChatMessage};

/// 合并开头的系统消息，并把后续系统更新转为用户上下文。
///
/// 参数:
/// - `messages`: 待发送给模型的消息列表
///
/// 返回:
/// - 保持历史前缀稳定的消息列表
pub(super) fn system_messages_first(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut system_parts = Vec::new();
    let mut other_messages = Vec::new();
    let mut leading_system = true;
    for message in messages {
        if leading_system && message.role == "system" {
            let text = chat_content_text(message.content);
            if !text.trim().is_empty() {
                system_parts.push(text);
            }
        } else {
            leading_system = false;
            if message.role == "system" {
                other_messages.push(ChatMessage::plain(
                    "user",
                    chat_content_text(message.content),
                ));
            } else {
                other_messages.push(message);
            }
        }
    }
    if system_parts.is_empty() {
        return other_messages;
    }
    let mut ordered = Vec::with_capacity(other_messages.len() + 1);
    ordered.push(ChatMessage::system(system_parts.join("\n\n")));
    ordered.extend(other_messages);
    ordered
}

/// 清理用户可见输入中的运行时提醒。
///
/// 参数:
/// - `input`: 原始用户输入
///
/// 返回:
/// - 清理后的用户输入
pub(super) fn clean_user_visible_text(input: &str) -> String {
    let mut output = input.to_string();
    for tag in ["system-reminder", "system_reminder"] {
        output = strip_tagged_sections(output, tag);
    }
    output
}

/// 提取消息文本内容。
///
/// 参数:
/// - `content`: 聊天消息内容
///
/// 返回:
/// - 文本内容，图片部分会被忽略
fn chat_content_text(content: Option<ChatContent>) -> String {
    match content {
        Some(ChatContent::Text(text)) => text,
        Some(ChatContent::Parts(parts)) => parts
            .into_iter()
            .filter_map(|part| match part {
                ChatContentPart::Text { text } => Some(text),
                ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

/// 删除指定 XML 标签包裹的内容。
///
/// 参数:
/// - `text`: 原始文本
/// - `tag`: 标签名称
///
/// 返回:
/// - 删除标签片段后的文本
fn strip_tagged_sections(mut text: String, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = text.find(&open) {
        let Some(relative_end) = text[start..].find(&close) else {
            text.replace_range(start.., "");
            break;
        };
        let end = start + relative_end + close.len();
        text.replace_range(start..end, "");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_pasted_system_reminder_from_user_input() {
        let input = "继续<system-reminder>hidden</system-reminder> ok";

        assert_eq!(clean_user_visible_text(input), "继续 ok");
    }

    #[test]
    fn strips_unclosed_system_reminder_from_user_input() {
        let input = "继续<system_reminder>hidden";

        assert_eq!(clean_user_visible_text(input), "继续");
    }

    #[test]
    fn dynamic_system_messages_stay_after_cached_history() {
        let messages = vec![
            ChatMessage::system("base"),
            ChatMessage::plain("user", "old input"),
            ChatMessage::plain("assistant", "old reply"),
            ChatMessage::system("runtime"),
            ChatMessage::plain("user", "new input"),
        ];

        let ordered = system_messages_first(messages);
        let roles = ordered
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>();
        let system_text = ordered
            .first()
            .and_then(|message| message.content.clone())
            .map(|content| chat_content_text(Some(content)))
            .unwrap();

        assert_eq!(roles, ["system", "user", "assistant", "user", "user"]);
        assert_eq!(system_text, "base");
        assert_eq!(chat_content_text(ordered[3].content.clone()), "runtime");
        assert_eq!(
            ordered
                .into_iter()
                .filter(|message| message.role == "system")
                .count(),
            1
        );
    }

    #[test]
    fn changing_runtime_context_preserves_the_cached_history_prefix() {
        let build = |runtime: &str| {
            system_messages_first(vec![
                ChatMessage::system("base"),
                ChatMessage::plain("user", "old input"),
                ChatMessage::plain("assistant", "old reply"),
                ChatMessage::system(runtime),
                ChatMessage::plain("user", "new input"),
            ])
        };

        let first = build("runtime one");
        let second = build("runtime two");
        let first_prefix = serde_json::to_value(&first[..3]).unwrap();
        let second_prefix = serde_json::to_value(&second[..3]).unwrap();

        assert_eq!(first_prefix, second_prefix);
    }

    #[test]
    fn system_normalization_keeps_messages_without_system() {
        let messages = vec![
            ChatMessage::plain("user", "old input"),
            ChatMessage::plain("assistant", "old reply"),
        ];

        let roles = system_messages_first(messages)
            .into_iter()
            .map(|message| message.role)
            .collect::<Vec<_>>();

        assert_eq!(roles, ["user", "assistant"]);
    }
}
