/// 交接摘要的可见前缀
const COMPACTION_SUMMARY_MARKER: &str = "<conversation-handoff>";

/// 构造注入对话上下文的交接笔记消息。
///
/// 摘要覆盖压缩边界内的历史，边界之后的用户、助手和工具消息继续完整保留。
/// 前缀明确告知模型：这是它自己的工作记录而非用户输入，且其中的完成
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
        已压缩的历史由本笔记覆盖；压缩边界之后的消息仍按原始顺序保留。\n\n\
        {body}\n\
        </conversation-handoff>"
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
}
