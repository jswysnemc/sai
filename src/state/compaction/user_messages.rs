use super::message_origin::{is_real_user_input, message_text, replace_message_text};
use crate::llm::ChatMessage;

/// 压缩后逐字保留的用户消息总预算（字符）。
///
/// sai 全程使用字符口径估算上下文，这里沿用同一单位避免两套换算。
pub const KEPT_USER_MESSAGE_MAX_CHARS: usize = 20_000;

/// 总预算中留给最早若干条用户消息的份额（字符）。
///
/// 最初的几条提问通常承载原始任务陈述，纯尾部选择会把它整个丢掉。
pub const KEPT_USER_MESSAGE_HEAD_CHARS: usize = 2_000;

/// 用户消息池的预算选择结果。
#[derive(Debug, Clone, Default)]
pub(crate) struct UserMessageSelection {
    /// 最早的若干条用户消息
    pub head: Vec<ChatMessage>,
    /// 最近的若干条用户消息
    pub tail: Vec<ChatMessage>,
    /// 头尾之间是否有内容被省略
    pub elided: bool,
    /// 被省略的字符数
    pub omitted_chars: usize,
}

/// 从完整历史中提取压缩后应逐字保留的用户消息。
///
/// 参数:
/// - `messages`: 压缩前的完整消息列表
///
/// 返回:
/// - 按原始顺序排列的真实用户输入
pub(crate) fn collect_compactable_user_messages(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    messages
        .iter()
        .filter(|message| is_real_user_input(message))
        .cloned()
        .collect()
}

/// 按头尾双段预算选择保留的用户消息。
///
/// 预算内全部保留；超出预算时头段保留最早的消息（任务陈述），尾段保留最近的
/// 消息（当前诉求）。边界消息按语义方向截断：尾段保留末尾、头段保留开头。
///
/// 参数:
/// - `messages`: 候选用户消息，按原始时间顺序
/// - `max_chars`: 总预算字符数
/// - `head_chars`: 头段预算字符数
///
/// 返回:
/// - 头段、尾段及省略统计
pub(crate) fn select_kept_user_messages(
    messages: &[ChatMessage],
    max_chars: usize,
    head_chars: usize,
) -> UserMessageSelection {
    let total = messages.iter().map(message_chars).sum::<usize>();
    // 1. 预算充足时整池保留
    if total <= max_chars {
        return UserMessageSelection {
            head: Vec::new(),
            tail: messages.to_vec(),
            elided: false,
            omitted_chars: 0,
        };
    }
    let head_budget = head_chars.min(max_chars);
    let tail_budget = max_chars - head_budget;
    // 2. 从最新往回填充尾段，边界消息保留末尾
    let (tail, head_end, boundary_prefix) = take_tail_within_budget(messages, tail_budget);
    // 3. 尾段边界被切掉的前半段回流为头段候选
    let mut head_candidates = messages[..head_end].to_vec();
    if let Some(prefix) = boundary_prefix {
        head_candidates.push(prefix);
    }
    // 4. 从最早往后填充头段，边界消息保留开头
    let head = take_head_within_budget(&head_candidates, head_budget);
    let kept = head.iter().chain(tail.iter()).map(message_chars).sum::<usize>();
    UserMessageSelection {
        head,
        tail,
        elided: true,
        omitted_chars: total.saturating_sub(kept),
    }
}

/// 从最新消息往回填充尾段。
///
/// 参数:
/// - `messages`: 候选用户消息
/// - `budget`: 尾段字符预算
///
/// 返回:
/// - 尾段消息、头段可用的结束下标、边界消息被切掉的前半段
fn take_tail_within_budget(
    messages: &[ChatMessage],
    budget: usize,
) -> (Vec<ChatMessage>, usize, Option<ChatMessage>) {
    let mut tail = Vec::new();
    let mut remaining = budget;
    let mut head_end = messages.len();
    let mut boundary_prefix = None;
    for (index, message) in messages.iter().enumerate().rev() {
        if remaining == 0 {
            break;
        }
        let chars = message_chars(message);
        if chars <= remaining {
            tail.push(message.clone());
            remaining -= chars;
            head_end = index;
            continue;
        }
        // 预算含义是"最近的字符"，边界消息保留末尾更贴合语义
        let Some(text) = message_text(message) else {
            break;
        };
        let kept_suffix = take_suffix_chars(&text, remaining);
        let dropped_len = text.chars().count() - kept_suffix.chars().count();
        if dropped_len > 0 {
            let prefix = text.chars().take(dropped_len).collect::<String>();
            boundary_prefix = Some(replace_message_text(message, prefix));
        }
        tail.push(replace_message_text(message, kept_suffix));
        head_end = index;
        break;
    }
    tail.reverse();
    (tail, head_end, boundary_prefix)
}

/// 从最早消息往后填充头段。
///
/// 参数:
/// - `messages`: 候选用户消息
/// - `budget`: 头段字符预算
///
/// 返回:
/// - 头段消息
fn take_head_within_budget(messages: &[ChatMessage], budget: usize) -> Vec<ChatMessage> {
    let mut head = Vec::new();
    let mut remaining = budget;
    for message in messages {
        if remaining == 0 {
            break;
        }
        let chars = message_chars(message);
        if chars <= remaining {
            head.push(message.clone());
            remaining -= chars;
            continue;
        }
        // 头段承载任务陈述，边界消息保留开头
        let Some(text) = message_text(message) else {
            break;
        };
        let kept = text.chars().take(remaining).collect::<String>();
        if !kept.is_empty() {
            head.push(replace_message_text(message, kept));
        }
        break;
    }
    head
}

/// 取文本末尾指定字符数。
///
/// 参数:
/// - `text`: 原始文本
/// - `chars`: 保留字符数
///
/// 返回:
/// - 末尾片段
fn take_suffix_chars(text: &str, chars: usize) -> String {
    let total = text.chars().count();
    if chars >= total {
        return text.to_string();
    }
    text.chars().skip(total - chars).collect()
}

/// 估算单条消息占用的字符数。
///
/// 参数:
/// - `message`: 待估算消息
///
/// 返回:
/// - 字符数估算
fn message_chars(message: &ChatMessage) -> usize {
    super::estimate_chat_messages_chars(std::slice::from_ref(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指定正文长度的 user 消息。
    ///
    /// 参数:
    /// - `marker`: 用于识别的首字符
    /// - `chars`: 正文字符数
    ///
    /// 返回:
    /// - user 角色消息
    fn user(marker: char, chars: usize) -> ChatMessage {
        ChatMessage::plain("user", marker.to_string().repeat(chars))
    }

    /// 验证预算充足时整池保留且不产生省略标记。
    #[test]
    fn keeps_all_messages_within_budget() {
        let messages = vec![user('a', 100), user('b', 100)];

        let selection = select_kept_user_messages(&messages, 20_000, 2_000);

        assert_eq!(selection.tail.len(), 2);
        assert!(selection.head.is_empty());
        assert!(!selection.elided);
    }

    /// 验证超预算时同时保留最早与最近的消息。
    #[test]
    fn keeps_head_and_tail_when_over_budget() {
        let messages = vec![
            user('a', 500),
            user('b', 500),
            user('c', 500),
            user('d', 500),
        ];

        let selection = select_kept_user_messages(&messages, 800, 200);

        assert!(selection.elided);
        assert!(!selection.head.is_empty(), "任务陈述必须保留");
        assert!(!selection.tail.is_empty(), "最近诉求必须保留");
        let head_text = message_text(&selection.head[0]).unwrap();
        assert!(head_text.starts_with('a'), "头段取最早消息");
        let tail_text = message_text(selection.tail.last().unwrap()).unwrap();
        assert!(tail_text.ends_with('d'), "尾段取最新消息");
    }

    /// 验证尾段边界消息保留末尾而非开头。
    #[test]
    fn tail_boundary_message_keeps_its_end() {
        let mut text = "x".repeat(400);
        text.push_str("ENDMARK");
        let messages = vec![ChatMessage::plain("user", text)];

        let selection = select_kept_user_messages(&messages, 200, 50);

        let tail_text = message_text(selection.tail.last().unwrap()).unwrap();
        assert!(tail_text.ends_with("ENDMARK"), "尾段必须保留末尾");
    }

    /// 验证尾段被切掉的前半段回流为头段候选。
    #[test]
    fn tail_boundary_prefix_flows_back_into_head() {
        let mut text = "HEADMARK".to_string();
        text.push_str(&"x".repeat(800));
        text.push_str("ENDMARK");
        let messages = vec![ChatMessage::plain("user", text)];

        let selection = select_kept_user_messages(&messages, 400, 100);

        let head_text = message_text(&selection.head[0]).unwrap();
        assert!(head_text.starts_with("HEADMARK"), "单条超大消息同时保住开头");
        let tail_text = message_text(selection.tail.last().unwrap()).unwrap();
        assert!(tail_text.ends_with("ENDMARK"), "同时保住结尾");
    }

    /// 验证省略字符数被如实统计。
    #[test]
    fn reports_omitted_chars() {
        let messages = (0..10).map(|_| user('a', 500)).collect::<Vec<_>>();

        let selection = select_kept_user_messages(&messages, 1_000, 200);

        assert!(selection.omitted_chars > 0);
    }

    /// 验证只提取真实用户输入。
    #[test]
    fn collects_only_real_user_input() {
        let messages = vec![
            ChatMessage::plain("user", "真实提问".to_string()),
            ChatMessage::plain("assistant", "回复".to_string()),
            ChatMessage::tool("call_1".to_string(), "结果".to_string()),
            ChatMessage::plain("user", "<runtime-context>\n注入".to_string()),
            ChatMessage::plain("user", "第二个提问".to_string()),
        ];

        let collected = collect_compactable_user_messages(&messages);

        assert_eq!(collected.len(), 2);
        assert_eq!(message_text(&collected[0]).as_deref(), Some("真实提问"));
        assert_eq!(message_text(&collected[1]).as_deref(), Some("第二个提问"));
    }
}
