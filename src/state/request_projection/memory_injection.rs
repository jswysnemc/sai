use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use crate::memory::file_store::MEMORY_TAG;
use std::collections::BTreeSet;

/// 构造本轮需要追加的记忆索引。
///
/// 内容没变就不必重发：它已经在历史里，模型看得到。本模型刚写入的条目
/// 同样跳过——模型自己知道写了什么。其它进程改了索引才追加差异；压缩
/// 丢掉正文后重新发送全文。
///
/// 参数:
/// - `messages`: 已投影的历史消息，不含当前轮
/// - `prompt`: 本轮准备注入的完整索引文本
///
/// 返回:
/// - 需要追加的索引或差异；无需注入时为 None
pub(super) fn memory_index_injection(messages: &[ChatMessage], prompt: &str) -> Option<String> {
    if memory_index_already_injected(messages, prompt) {
        return None;
    }
    let Some(previous) = latest_injected_index(messages) else {
        return Some(prompt.to_string());
    };
    if model_wrote_memory_since_last_index(messages) {
        return None;
    }
    Some(memory_index_delta(&previous, prompt).unwrap_or_else(|| prompt.to_string()))
}

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
/// - 最近一次注入的索引与本次完全相同，或差异块的目标哈希一致时为 true
pub(super) fn memory_index_already_injected(messages: &[ChatMessage], prompt: &str) -> bool {
    if latest_injected_index(messages).as_deref() == Some(prompt) {
        return true;
    }
    latest_memory_target_hash(messages).as_deref() == Some(content_hash(prompt).as_str())
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

/// 取出最近一次注入所代表的完整索引哈希。
///
/// 参数:
/// - `messages`: 已投影的历史消息
///
/// 返回:
/// - 差异块上的目标哈希；全文注入或没有哈希时为 None
fn latest_memory_target_hash(messages: &[ChatMessage]) -> Option<String> {
    hash_from_open_tag(&latest_injected_index(messages)?)
}

/// 从消息文本中截出记忆索引块。
///
/// 参数:
/// - `text`: 消息纯文本
///
/// 返回:
/// - 含首尾标签的索引块；不含完整标签对时为 None
fn extract_memory_block(text: &str) -> Option<String> {
    let open_prefix = format!("<{MEMORY_TAG}");
    let close = format!("</{MEMORY_TAG}>");
    let start = text.find(&open_prefix)?;
    let after = &text[start + open_prefix.len()..];
    if !after.starts_with('>') && !after.starts_with(' ') && !after.starts_with('\t') {
        return None;
    }
    let end = text[start..].find(&close)? + start + close.len();
    Some(text[start..end].to_string())
}

/// 判断上一份索引之后，当前会话的模型是否调用过 write_memory。
///
/// 参数:
/// - `messages`: 已投影的历史消息
///
/// 返回:
/// - 最近一次索引之后存在 write_memory 调用时为 true
fn model_wrote_memory_since_last_index(messages: &[ChatMessage]) -> bool {
    let start = messages
        .iter()
        .rposition(|message| {
            message.role == "user" && extract_memory_block(&message_text(message)).is_some()
        })
        .map(|index| index + 1)
        .unwrap_or(0);
    messages[start..].iter().any(message_wrote_memory)
}

/// 判断一条助手消息是否调用了 write_memory。
///
/// 参数:
/// - `message`: 聊天消息
///
/// 返回:
/// - 工具调用列表含 write_memory 时为 true
fn message_wrote_memory(message: &ChatMessage) -> bool {
    message.tool_calls.as_ref().is_some_and(|calls| {
        calls
            .iter()
            .any(|call| call.function.name == "write_memory")
    })
}

/// 把两份索引的条目差写成增量块。
///
/// 参数:
/// - `previous`: 上一份含标签的索引
/// - `current`: 当前完整索引
///
/// 返回:
/// - 可解析的增减块；条目无法对齐或变化过大时为 None，调用方改发全文
fn memory_index_delta(previous: &str, current: &str) -> Option<String> {
    let old = index_entry_lines(previous);
    let new = index_entry_lines(current);
    if old == new {
        return None;
    }
    let added = new.difference(&old).cloned().collect::<Vec<_>>();
    let removed = old.difference(&new).cloned().collect::<Vec<_>>();
    if added.is_empty() && removed.is_empty() {
        return None;
    }
    if new.len() > 8 && added.len() + removed.len() > new.len() / 2 {
        return None;
    }
    let hash = content_hash(current);
    let mut output = format!(
        "<{MEMORY_TAG} delta=\"true\" hash=\"{hash}\">\n记忆索引有更新，在上一份完整索引上应用以下增减。\n"
    );
    if !added.is_empty() {
        output.push_str("\nadded:\n");
        for line in &added {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !removed.is_empty() {
        output.push_str("\nremoved:\n");
        for line in &removed {
            output.push_str(line);
            output.push('\n');
        }
    }
    output.push_str(&format!("</{MEMORY_TAG}>"));
    Some(output)
}

/// 取出索引块里的条目行。
///
/// 参数:
/// - `block`: 含标签的索引文本
///
/// 返回:
/// - 以 `- [` 开头的条目
fn index_entry_lines(block: &str) -> BTreeSet<String> {
    block
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- ["))
        .map(str::to_string)
        .collect()
}

/// 从起始标签读取目标哈希。
///
/// 参数:
/// - `block`: 含标签的索引块
///
/// 返回:
/// - `hash="..."` 属性值
fn hash_from_open_tag(block: &str) -> Option<String> {
    let open_end = block.find('>')?;
    let open = &block[..=open_end];
    let key = "hash=\"";
    let start = open.find(key)? + key.len();
    let end = open[start..].find('"')? + start;
    Some(open[start..end].to_string())
}

/// 计算索引正文指纹。
///
/// 参数:
/// - `text`: 完整索引文本
///
/// 返回:
/// - 16 位十六进制哈希
fn content_hash(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex()[..16].to_string()
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
    use crate::llm::{ToolCall, ToolCallFunction};

    /// 构造一条带索引的用户消息。
    fn user_with_index(index: &str, input: &str) -> ChatMessage {
        ChatMessage::plain("user", format!("{index}\n\n{input}"))
    }

    /// 构造一份索引注入文本。
    fn index(body: &str) -> String {
        format!("<{MEMORY_TAG}>\n{body}\n</{MEMORY_TAG}>")
    }

    /// 构造一条调用 write_memory 的助手消息。
    fn wrote_memory() -> ChatMessage {
        ChatMessage::assistant(
            "",
            Some(vec![ToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: ToolCallFunction {
                    name: "write_memory".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        )
    }

    /// 验证内容相同的索引被认定为已注入。
    #[test]
    fn an_unchanged_index_counts_as_injected() {
        let current = index("- [一](a.md)");
        let history = vec![user_with_index(&current, "上一轮输入")];

        assert!(memory_index_already_injected(&history, &current));
        assert!(memory_index_injection(&history, &current).is_none());
    }

    /// 验证索引变化后重新注入。
    ///
    /// 漏掉这一步，新记忆写完当轮就再也进不了上下文。
    #[test]
    fn a_changed_index_is_injected_again() {
        let history = vec![user_with_index(&index("- [一](a.md)"), "上一轮输入")];
        let next = index("- [一](a.md)\n- [二](b.md)");

        assert!(!memory_index_already_injected(&history, &next));
        let injected = memory_index_injection(&history, &next).unwrap();
        assert!(injected.contains("delta=\"true\""));
        assert!(injected.contains("- [二](b.md)"));
        assert!(!injected.contains("- [一](a.md)"));
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
        let current = index("- [一](a.md)");

        assert!(!memory_index_already_injected(&history, &current));
        assert_eq!(
            memory_index_injection(&history, &current).as_deref(),
            Some(current.as_str())
        );
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

    /// 验证本模型刚写入记忆后不再把同一份索引追加回去。
    #[test]
    fn skips_reinjection_when_this_model_just_wrote_memory() {
        let previous = index("- [一](a.md)");
        let current = index("- [一](a.md)\n- [二](b.md)");
        let history = vec![
            user_with_index(&previous, "上一轮输入"),
            wrote_memory(),
            ChatMessage::plain("tool", "ok"),
        ];

        assert!(memory_index_injection(&history, &current).is_none());
    }

    /// 验证差异注入后按目标哈希认定为已同步。
    #[test]
    fn a_delta_hash_counts_as_the_current_index() {
        let previous = index("- [一](a.md)");
        let current = index("- [一](a.md)\n- [二](b.md)");
        let delta =
            memory_index_injection(&[user_with_index(&previous, "上一轮")], &current).unwrap();
        let history = vec![user_with_index(&delta, "本轮")];

        assert!(memory_index_already_injected(&history, &current));
        assert!(memory_index_injection(&history, &current).is_none());
    }
}
