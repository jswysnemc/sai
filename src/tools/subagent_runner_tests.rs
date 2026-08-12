use super::subagent_runner::{
    estimate_tokens, format_token_count, wrap_subagent_inbox, ProgressMode, SubagentProgress,
};
use super::ToolProgress;
use tokio::sync::mpsc;

/// 验证子 Agent 用量回退与全局 BPE 估算保持一致。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn estimates_tokens_with_shared_bpe() {
    assert_eq!(estimate_tokens(&["abcd"]), 1);
    assert_eq!(estimate_tokens(&["abcdefgh"]), 1);
    assert_eq!(estimate_tokens(&["你好世界"]), 2);
    assert_eq!(estimate_tokens(&["hello ", "world"]), 2);
}

/// 验证 Token 数量格式不依赖非 ASCII 前缀。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn formats_token_counts_without_unicode_prefix() {
    assert_eq!(format_token_count(999, false), "999");
    assert_eq!(format_token_count(1_500, true), "~1.5k");
}

/// 验证完整进度模式会立即转发子 Agent 正文分片。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn full_progress_forwards_content_chunks_immediately() {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let progress = SubagentProgress::new(ToolProgress::new(sender), ProgressMode::Full, true);

    progress.content("first");
    progress.content(" second");

    assert_eq!(receiver.try_recv().unwrap(), "__subagent_text__first");
    assert_eq!(receiver.try_recv().unwrap(), "__subagent_text__ second");
}

/// 【消息注入】验证追加消息的注入文本带来源标记且保留正文。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn inbox_wrapper_marks_message_source() {
    let wrapped = wrap_subagent_inbox("user", "优先修复测试");

    assert!(wrapped.starts_with("<subagent-inbox from=\"user\">"));
    assert!(wrapped.contains("优先修复测试"));
    assert!(wrapped.ends_with("</subagent-inbox>"));
    assert!(wrap_subagent_inbox("parent", "x").contains("from=\"parent\""));
}
