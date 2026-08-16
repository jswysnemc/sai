use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use crate::memory::file_store::MEMORY_TAG;

/// 判断这份记忆索引是否已经原样注入过。
///
/// 索引每轮全量注入会让同一份内容在上下文里堆叠 N 份。它并不影响前缀缓存
/// ——历史里存的是当轮实际发送的内容，重放时字节一致——但重复的部分要一直
/// 占着窗口，而且早期快照里可能还留着此后已经删掉的条目，读起来自相矛盾。
///
/// 内容没变就不必重发：它已经在历史里，模型看得到。变了才注入，最新的那份
/// 也就自然落在离当前输入最近的位置。
///
/// 参数:
/// - `messages`: 已投影的历史消息，不含当前轮
/// - `prompt`: 本轮准备注入的索引文本
///
/// 返回:
/// - 最近一次注入的索引与本次完全相同时为 true
pub(super) fn memory_index_already_injected(messages: &[ChatMessage], prompt: &str) -> bool {
    latest_injected_index(messages).is_some_and(|latest| latest == prompt)
}

/// 取出历史中最后一次注入的索引文本。
///
/// 只看最后一次：中间轮次的旧索引已被更新的那份取代，与本轮是否重复无关。
///
/// 参数:
/// - `messages`: 已投影的历史消息
///
/// 返回:
/// - 最后一个用户消息里的索引块；从未注入过时为 None
fn latest_injected_index(messages: &[ChatMessage]) -> Option<String> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == "user")
        .find_map(|message| extract_memory_block(&message_text(message)))
}

/// 从消息文本中截出记忆索引块。
///
/// 参数:
/// - `text`: 消息纯文本
///
/// 返回:
/// - 含首尾标签的索引块；不含完整标签对时为 None
fn extract_memory_block(text: &str) -> Option<String> {
    let open = format!("<{MEMORY_TAG}>");
    let close = format!("</{MEMORY_TAG}>");
    let start = text.find(&open)?;
    // 从起始标签之后再找闭合标签，避免正文里出现的同名字样把范围截错
    let end = text[start..].find(&close)? + start + close.len();
    Some(text[start..end].to_string())
}

/// 取出消息的纯文本内容。
///
/// 带图片的消息其文本落在 Parts 里，只认 Text 变体会漏掉这类消息，
/// 于是每次发图都白白重发一份索引。
///
/// 参数:
/// - `message`: 聊天消息
///
/// 返回:
/// - 拼接后的文本；没有文本内容时为空串
fn message_text(message: &ChatMessage) -> String {
    match message.content.as_ref() {
        Some(ChatContent::Text(text)) => text.clone(),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                ChatContentPart::Text { text } => Some(text.as_str()),
                ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条带索引的用户消息。
    fn user_with_index(index: &str, input: &str) -> ChatMessage {
        ChatMessage::plain("user", format!("{index}\n\n{input}"))
    }

    /// 构造一份索引注入文本。
    fn index(body: &str) -> String {
        format!("<{MEMORY_TAG}>\n{body}\n</{MEMORY_TAG}>")
    }

    /// 验证内容相同的索引被认定为已注入。
    #[test]
    fn an_unchanged_index_counts_as_injected() {
        let current = index("- [一](a.md)");
        let history = vec![user_with_index(&current, "上一轮输入")];

        assert!(memory_index_already_injected(&history, &current));
    }

    /// 验证索引变化后重新注入。
    ///
    /// 漏掉这一步，新记忆写完当轮就再也进不了上下文。
    #[test]
    fn a_changed_index_is_injected_again() {
        let history = vec![user_with_index(&index("- [一](a.md)"), "上一轮输入")];

        assert!(!memory_index_already_injected(
            &history,
            &index("- [一](a.md)\n- [二](b.md)")
        ));
    }

    /// 验证历史里没有索引时判定为未注入。
    ///
    /// 压缩会把历史换成摘要，其中的索引块随之消失，此时必须重新注入。
    #[test]
    fn a_history_without_any_index_counts_as_not_injected() {
        let history = vec![
            ChatMessage::plain("user", "纯输入"),
            ChatMessage::plain("assistant", "回复"),
        ];

        assert!(!memory_index_already_injected(
            &history,
            &index("- [一](a.md)")
        ));
    }

    /// 验证只与最近一次注入比较。
    ///
    /// 拿旧快照比会让当前索引被误判成新内容，于是每轮都重发。
    #[test]
    fn only_the_most_recent_injection_is_compared() {
        let current = index("- [一](a.md)\n- [二](b.md)");
        let history = vec![
            user_with_index(&index("- [一](a.md)"), "第一轮"),
            ChatMessage::plain("assistant", "回复"),
            user_with_index(&current, "第二轮"),
        ];

        assert!(memory_index_already_injected(&history, &current));
    }

    /// 验证带图片的消息同样能取出索引。
    #[test]
    fn an_index_inside_an_image_message_is_found() {
        let current = index("- [一](a.md)");
        let history = vec![ChatMessage::user_with_image(
            format!("{current}\n\n看图"),
            "https://example.invalid/a.png",
        )];

        assert!(memory_index_already_injected(&history, &current));
    }

    /// 验证只有助手消息提到标签时不算注入。
    ///
    /// 模型复述索引不代表系统注入过，据此跳过会让索引彻底不进上下文。
    #[test]
    fn an_assistant_mention_does_not_count() {
        let current = index("- [一](a.md)");
        let history = vec![ChatMessage::plain(
            "assistant",
            format!("我看到了 {current}"),
        )];

        assert!(!memory_index_already_injected(&history, &current));
    }

    /// 验证缺少闭合标签时不误判。
    #[test]
    fn an_unterminated_block_is_ignored() {
        let history = vec![ChatMessage::plain(
            "user",
            format!("<{MEMORY_TAG}>\n- [一](a.md)"),
        )];

        assert!(!memory_index_already_injected(
            &history,
            &index("- [一](a.md)")
        ));
    }
}
