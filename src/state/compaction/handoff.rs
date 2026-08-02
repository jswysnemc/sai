use super::message_origin::{COMPACTION_ELISION_MARKER, COMPACTION_SUMMARY_MARKER};

/// 构造注入对话上下文的交接笔记消息。
///
/// 压缩后模型只能看到保留的用户消息和这条笔记，全部 assistant 与 tool 消息都
/// 已移除。前缀明确告知模型：这是它自己的工作记录而非用户输入，且其中的完成
/// 声明未经验证，不能直接当作既成事实。
///
/// 参数:
/// - `summary`: 压缩模型生成的交接笔记正文
///
/// 返回:
/// - 可直接作为 user 消息注入的完整文本
pub fn summary_context_message(summary: &str) -> String {
    let body = summary.trim();
    let body = if body.is_empty() {
        "(交接笔记为空)"
    } else {
        body
    };
    format!(
        "{COMPACTION_SUMMARY_MARKER}\n\
        为释放上下文，此前的对话已被压缩。以下是你自己为这项任务写的工作笔记，\
        用它接续原有的思路，不要从头重来。\n\
        把它当作笔记而非证据：凡是笔记中声称某步已完成、测试已通过、问题已修复的，\
        都要先自行验证再依赖。\n\
        此上下文中更早的用户消息按原文保留；若其中出现省略标记，被省略的用户消息\
        已由本笔记覆盖。\n\n\
        {body}\n\
        </conversation-handoff>"
    )
}

/// 构造用户消息池的省略标记。
///
/// 用户消息总量超出保留预算时插在头尾两段之间，说明中间的提问已由交接笔记覆盖。
///
/// 参数:
/// - `omitted_chars`: 被省略的字符数
///
/// 返回:
/// - 可直接作为 user 消息注入的标记文本
pub(crate) fn elision_marker_message(omitted_chars: usize) -> String {
    format!(
        "{COMPACTION_ELISION_MARKER} chars=\"{omitted_chars}\">\n\
        此处省略了中间若干条用户消息，其内容已包含在下方的交接笔记中。\n\
        </omitted-user-messages>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证摘要消息带可识别前缀，供下次压缩排除。
    #[test]
    fn summary_message_carries_recognizable_marker() {
        let message = summary_context_message("我正在重构 X 模块");

        assert!(message.starts_with(COMPACTION_SUMMARY_MARKER));
        assert!(message.contains("我正在重构 X 模块"));
    }

    /// 验证摘要消息提示模型自行验证完成声明。
    #[test]
    fn summary_message_warns_against_trusting_claims() {
        let message = summary_context_message("测试已通过");

        assert!(message.contains("先自行验证"));
    }

    /// 验证空摘要有占位文本。
    #[test]
    fn empty_summary_has_placeholder() {
        let message = summary_context_message("   ");

        assert!(message.contains("交接笔记为空"));
    }

    /// 验证省略标记带可识别前缀与字符数。
    #[test]
    fn elision_marker_reports_omitted_chars() {
        let marker = elision_marker_message(1_234);

        assert!(marker.starts_with(COMPACTION_ELISION_MARKER));
        assert!(marker.contains("1234"));
    }
}
