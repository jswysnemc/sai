use crate::i18n::text as t;

/// 摘要末尾回读指引的分隔线。
const POINTER_SEPARATOR: &str = "---";

/// 生成摘要末尾的原文回读指引。
///
/// 摘要是有损的，但被压缩的轮次并没有从库里删除。不写这一句，下一轮
/// 无从知道原文还在，只能凭摘要推测；写上之后，缺细节时可以回去查。
///
/// 参数:
/// - `session_id`: 当前会话标识
/// - `lookup_available`: 回读工具是否确实注册可用
///
/// 返回:
/// - 回读指引文本；工具不可用时返回 None，避免指向一个不存在的能力
pub(crate) fn transcript_pointer(session_id: &str, lookup_available: bool) -> Option<String> {
    if !lookup_available || session_id.trim().is_empty() {
        return None;
    }
    let template = t(
        "This summary covers turns cleared from session {session}. The originals were not deleted: when a detail is missing, retrieve it with the search_evicted_context tool instead of guessing from this summary.",
        "本摘要覆盖会话 {session} 中已清出上下文的轮次。原文并未删除：缺具体细节时用 search_evicted_context 工具按关键词回读，不要凭摘要推测。",
    );
    Some(format!(
        "{POINTER_SEPARATOR}\n{}",
        template.replace("{session}", session_id.trim())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证工具不可用时不产生指引。
    ///
    /// 指向一个未注册的工具比不提示更糟：下一轮会尝试调用并失败。
    #[test]
    fn no_pointer_when_the_lookup_tool_is_absent() {
        assert!(transcript_pointer("s-1", false).is_none());
    }

    /// 验证会话标识为空时不产生指引。
    #[test]
    fn no_pointer_without_a_session_id() {
        assert!(transcript_pointer("   ", true).is_none());
    }

    /// 验证指引带上会话标识与工具名。
    #[test]
    fn pointer_names_both_the_session_and_the_tool() {
        let pointer = transcript_pointer("s-42", true).unwrap();

        assert!(pointer.contains("s-42"));
        assert!(pointer.contains("search_evicted_context"));
    }
}
