use super::model::{RuntimeRecoveryKind, RuntimeRecoveryStatus};

/// 命令摘要使用的 Runtime Recovery 投影。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeRecoverySummary {
    pub active_process_count: usize,
    pub stale_process_count: usize,
    pub latest_failure: Option<RuntimeRecoveryFailureSummary>,
}

/// 命令摘要使用的最近运行时恢复失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRecoveryFailureSummary {
    pub process_id: Option<String>,
    pub kind: RuntimeRecoveryKind,
    pub status: RuntimeRecoveryStatus,
    pub reason: String,
    pub last_safe_seq: Option<i64>,
    pub created_at: String,
}

/// 判断 Runtime Recovery 摘要是否需要展示。
///
/// 参数:
/// - `summary`: Runtime Recovery 摘要
///
/// 返回:
/// - 是否存在用户可见状态
pub(crate) fn has_visible_runtime_recovery(summary: &RuntimeRecoverySummary) -> bool {
    summary.active_process_count > 0
        || summary.stale_process_count > 0
        || summary.latest_failure.is_some()
}
