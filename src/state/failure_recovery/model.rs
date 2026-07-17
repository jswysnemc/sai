/// 恢复记录类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    CompactionLlmFailed,
    EmptySummary,
    CompactionOverBudget,
    CompactionMirrorFailed,
    ProviderOverflow,
    OverflowRetryFailed,
    StaleRunningTurn,
    ProjectionInvalid,
    SessionMemoryExtractionFailed,
    SessionMemoryBoundaryInvalid,
    SessionMemoryCompactFailed,
    ToolHistoryMissingResult,
    ToolHistoryOrphanResult,
    ToolHistoryDuplicateResult,
    ToolHistoryPendingStale,
    ToolHistoryReplacementMissing,
    ToolHistoryPromptOverBudget,
}

impl FailureKind {
    /// 转换为数据库文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 数据库文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::CompactionLlmFailed => "compaction_llm_failed",
            Self::EmptySummary => "empty_summary",
            Self::CompactionOverBudget => "compaction_over_budget",
            Self::CompactionMirrorFailed => "compaction_mirror_failed",
            Self::ProviderOverflow => "provider_overflow",
            Self::OverflowRetryFailed => "overflow_retry_failed",
            Self::StaleRunningTurn => "stale_running_turn",
            Self::ProjectionInvalid => "projection_invalid",
            Self::SessionMemoryExtractionFailed => "session_memory_extraction_failed",
            Self::SessionMemoryBoundaryInvalid => "session_memory_boundary_invalid",
            Self::SessionMemoryCompactFailed => "session_memory_compact_failed",
            Self::ToolHistoryMissingResult => "tool_history_missing_result",
            Self::ToolHistoryOrphanResult => "tool_history_orphan_result",
            Self::ToolHistoryDuplicateResult => "tool_history_duplicate_result",
            Self::ToolHistoryPendingStale => "tool_history_pending_stale",
            Self::ToolHistoryReplacementMissing => "tool_history_replacement_missing",
            Self::ToolHistoryPromptOverBudget => "tool_history_prompt_over_budget",
        }
    }

    /// 从数据库文本恢复类型。
    ///
    /// 参数:
    /// - `value`: 数据库文本
    ///
    /// 返回:
    /// - 恢复记录类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "empty_summary" => Self::EmptySummary,
            "compaction_over_budget" => Self::CompactionOverBudget,
            "compaction_mirror_failed" => Self::CompactionMirrorFailed,
            "provider_overflow" => Self::ProviderOverflow,
            "overflow_retry_failed" => Self::OverflowRetryFailed,
            "stale_running_turn" => Self::StaleRunningTurn,
            "projection_invalid" => Self::ProjectionInvalid,
            "session_memory_extraction_failed" => Self::SessionMemoryExtractionFailed,
            "session_memory_boundary_invalid" => Self::SessionMemoryBoundaryInvalid,
            "session_memory_compact_failed" => Self::SessionMemoryCompactFailed,
            "tool_history_missing_result" => Self::ToolHistoryMissingResult,
            "tool_history_orphan_result" => Self::ToolHistoryOrphanResult,
            "tool_history_duplicate_result" => Self::ToolHistoryDuplicateResult,
            "tool_history_pending_stale" => Self::ToolHistoryPendingStale,
            "tool_history_replacement_missing" => Self::ToolHistoryReplacementMissing,
            "tool_history_prompt_over_budget" => Self::ToolHistoryPromptOverBudget,
            _ => Self::CompactionLlmFailed,
        }
    }
}

/// 恢复记录状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryStatus {
    Observed,
    Recovering,
    Reprojected,
    Resolved,
    Terminal,
}

impl RecoveryStatus {
    /// 转换为数据库文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 数据库文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Recovering => "recovering",
            Self::Reprojected => "reprojected",
            Self::Resolved => "resolved",
            Self::Terminal => "terminal",
        }
    }

    /// 从数据库文本恢复状态。
    ///
    /// 参数:
    /// - `value`: 数据库文本
    ///
    /// 返回:
    /// - 恢复状态
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "recovering" => Self::Recovering,
            "reprojected" => Self::Reprojected,
            "resolved" => Self::Resolved,
            "terminal" => Self::Terminal,
            _ => Self::Observed,
        }
    }
}

/// 会话恢复记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryRecord {
    pub id: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    pub kind: FailureKind,
    pub status: RecoveryStatus,
    pub reason: String,
    pub retry_count: usize,
    pub checkpoint_id: Option<String>,
    pub context_chars: usize,
    pub context_limit_chars: usize,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// 待写入的恢复记录。
#[derive(Debug, Clone)]
pub(crate) struct NewRecoveryRecord {
    pub session_id: String,
    pub turn_id: Option<String>,
    pub kind: FailureKind,
    pub status: RecoveryStatus,
    pub reason: String,
    pub retry_count: usize,
    pub checkpoint_id: Option<String>,
    pub context_chars: usize,
    pub context_limit_chars: usize,
}

/// 会话恢复快照。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoverySnapshot {
    pub latest: Option<RecoveryRecord>,
    pub auto_compaction_failures: usize,
    pub auto_compaction_blocked: bool,
    pub stale_turns_recovered: usize,
}
