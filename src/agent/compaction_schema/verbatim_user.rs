use super::catalog::machine_filled_section;
use crate::state::Turn;

/// 单条用户消息至少保留的字符数。
///
/// 宁可每条都被截短，也不让任何一条整体消失：这一节的作用是让下一轮
/// 知道"用户到底提过哪些事"，少一条就等于那个请求从未存在过。
const MIN_CHARS_PER_MESSAGE: usize = 120;

/// 整节的默认字符预算。
pub(crate) const DEFAULT_USER_SECTION_BUDGET: usize = 8_000;

/// 截断标注的模板前缀。
const TRUNCATION_MARK: &str = "……〔中间省略";

/// 把被压缩轮次的用户原话渲染成机器填充节。
///
/// 只收被压缩掉的那些轮次：仍留在上下文里的近期消息模型本来就看得见，
/// 重复搬进摘要只是浪费预算。
///
/// 参数:
/// - `turns`: 本次被压缩的轮次
/// - `budget`: 整节可用的字符预算
///
/// 返回:
/// - 含标题的完整小节文本；没有任何用户原话时仍返回标题与说明行
pub(crate) fn user_messages_section(turns: &[Turn], budget: usize) -> String {
    let heading = machine_filled_section().heading();
    let messages = collect_messages(turns);
    if messages.is_empty() {
        return format!("{heading}\n（本次压缩区间内没有用户消息）");
    }
    let allowances = allocate(&messages, budget);
    let body = messages
        .iter()
        .zip(allowances)
        .map(|(message, allowance)| format!("- {}", quote(message, allowance)))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{heading}\n{body}")
}

/// 收集非空的用户原话。
///
/// 参数:
/// - `turns`: 被压缩的轮次
///
/// 返回:
/// - 按轮次顺序排列的用户消息原文
fn collect_messages(turns: &[Turn]) -> Vec<&str> {
    turns
        .iter()
        .map(|turn| turn.user_content.trim())
        .filter(|content| !content.is_empty())
        .collect()
}

/// 按预算给每条消息分配可用字符数。
///
/// 分两轮：先让短消息全额保留，把它们用不完的额度回收；再把回收后的
/// 余量平分给超额的长消息。一轮回收已经能让"一条长消息 + 若干短消息"
/// 这种常见形态得到合理分配，不必迭代到收敛。
///
/// 参数:
/// - `messages`: 全部用户消息
/// - `budget`: 整节字符预算
///
/// 返回:
/// - 与消息一一对应的字符额度
fn allocate(messages: &[&str], budget: usize) -> Vec<usize> {
    let lengths: Vec<usize> = messages.iter().map(|message| message.chars().count()).collect();
    let total: usize = lengths.iter().sum();
    // 1. 预算足够时全量保留，不做任何截断
    if total <= budget {
        return lengths;
    }
    // 2. 保底额度已经超预算时，所有消息一律按保底截断
    let floor = MIN_CHARS_PER_MESSAGE.min(budget / messages.len().max(1));
    if floor * messages.len() >= budget {
        return vec![floor.max(1); messages.len()];
    }
    // 3. 短消息全额保留，剩余额度平分给超额的长消息
    let fair = budget / messages.len();
    let kept: usize = lengths.iter().filter(|length| **length <= fair).sum();
    let long_count = lengths.iter().filter(|length| **length > fair).count();
    let share = if long_count == 0 {
        fair
    } else {
        (budget.saturating_sub(kept) / long_count).max(floor)
    };
    lengths
        .iter()
        .map(|length| if *length <= fair { *length } else { share })
        .collect()
}

/// 按额度渲染一条消息，超额时保头保尾截断。
///
/// 用户消息通常开头是要求、结尾是补充或纠正，中间最可省。只保开头
/// 会把"另外还有一点"这类追加要求整段丢掉。
///
/// 参数:
/// - `message`: 消息原文
/// - `allowance`: 该条可用字符数
///
/// 返回:
/// - 渲染后的单行文本；换行被替换为可见标记以保持一行一条
fn quote(message: &str, allowance: usize) -> String {
    let chars: Vec<char> = message.chars().collect();
    if chars.len() <= allowance {
        return escape_newlines(message);
    }
    let omitted = chars.len() - allowance;
    let head_len = allowance * 2 / 3;
    let tail_len = allowance.saturating_sub(head_len);
    let head: String = chars.iter().take(head_len).collect();
    let tail: String = chars[chars.len() - tail_len..].iter().collect();
    format!(
        "{}{TRUNCATION_MARK} {omitted} 字符〕{}",
        escape_newlines(&head),
        escape_newlines(&tail)
    )
}

/// 把换行替换为可见标记。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 单行文本
fn escape_newlines(value: &str) -> String {
    value.replace("\r\n", "\\n").replace(['\n', '\r'], "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TurnStatus;

    /// 构造一个只关心用户原话的轮次。
    ///
    /// 参数:
    /// - `content`: 用户原话
    ///
    /// 返回:
    /// - 测试轮次
    fn turn(content: &str) -> Turn {
        Turn {
            turn_id: "t".to_string(),
            seq: 1,
            user_content: content.to_string(),
            user_image_urls: Vec::new(),
            user_timestamp: String::new(),
            assistant_content: String::new(),
            assistant_reasoning: None,
            assistant_timestamp: None,
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
            duration_ms: 0,
            parent_turn_id: None,
            model: None,
            error: None,
        }
    }

    /// 验证预算充足时逐字保留每条消息。
    ///
    /// 这一节存在的全部理由就是零失真，预算够却改写等于自毁。
    #[test]
    fn every_message_is_kept_verbatim_within_budget() {
        let section = user_messages_section(&[turn("第一条"), turn("第二条")], 1_000);

        assert!(section.contains("- 第一条"));
        assert!(section.contains("- 第二条"));
    }

    /// 验证换行被折成单行标记。
    #[test]
    fn newlines_are_folded_into_one_line() {
        let section = user_messages_section(&[turn("tui:\n1.折叠\n2.展开")], 1_000);

        assert!(section.contains("tui:\\n1.折叠\\n2.展开"));
        assert_eq!(section.lines().count(), 2);
    }

    /// 验证预算不足时没有任何一条消息整体消失。
    #[test]
    fn no_message_disappears_under_pressure() {
        let turns: Vec<Turn> = (0..10).map(|index| turn(&"长".repeat(500 + index))).collect();

        let section = user_messages_section(&turns, 600);

        assert_eq!(section.lines().count(), 11);
    }

    /// 验证截断保留开头与结尾。
    ///
    /// 用户的追加要求常在末尾，只保开头会把它整段丢掉。
    #[test]
    fn truncation_keeps_head_and_tail() {
        let message = format!("开头要求{}结尾补充", "填".repeat(500));

        let section = user_messages_section(&[turn(&message)], 200);

        assert!(section.contains("开头要求"));
        assert!(section.contains("结尾补充"));
        assert!(section.contains(TRUNCATION_MARK));
    }

    /// 验证短消息不会被长消息挤掉额度。
    #[test]
    fn short_messages_survive_next_to_a_long_one() {
        let turns = vec![turn("短"), turn(&"长".repeat(5_000)), turn("也短")];

        let section = user_messages_section(&turns, 1_000);

        assert!(section.contains("- 短"));
        assert!(section.contains("- 也短"));
    }

    /// 验证空内容轮次不产生空行条目。
    #[test]
    fn blank_turns_are_skipped() {
        let section = user_messages_section(&[turn("   "), turn("有内容")], 1_000);

        assert_eq!(section.lines().count(), 2);
        assert!(section.contains("- 有内容"));
    }

    /// 验证没有任何用户原话时仍产出标题。
    ///
    /// 缺标题会让后续的插入定位失效。
    #[test]
    fn heading_survives_an_empty_interval() {
        let section = user_messages_section(&[], 1_000);

        assert!(section.starts_with(&machine_filled_section().heading()));
    }

    /// 验证截断发生在字符边界上。
    ///
    /// 按字节切多字节字符会直接 panic。
    #[test]
    fn truncation_respects_character_boundaries() {
        let section = user_messages_section(&[turn(&"中文字符".repeat(400))], 150);

        assert!(section.chars().count() > 0);
    }
}
