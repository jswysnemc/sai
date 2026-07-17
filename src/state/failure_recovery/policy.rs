use super::model::RecoverySnapshot;

pub(crate) const AUTO_COMPACTION_FAILURE_THRESHOLD: usize = 3;

/// 判断是否允许尝试自动压缩。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 是否允许自动压缩
pub(crate) fn should_attempt_auto_compaction(snapshot: &RecoverySnapshot) -> bool {
    !snapshot.auto_compaction_blocked
}

/// 计算下一次自动压缩失败次数。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 下一次连续失败次数
pub(crate) fn next_auto_compaction_retry_count(snapshot: &RecoverySnapshot) -> usize {
    snapshot.auto_compaction_failures + 1
}
