use crate::llm::ChatMessage;
use crate::state::compaction::CompactionSummary;
use crate::state::usage::UsageSnapshot;
use crate::state::RecoverySnapshot;

/// 投影请求类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectionKind {
    ProviderTurn,
    SessionSummary,
}

/// 请求或摘要投影的上下文估算。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ProjectionEstimate {
    pub message_chars: usize,
    pub state_context_chars: usize,
    pub context_limit_chars: usize,
    pub context_ratio: f32,
}

/// 投影来源统计。
#[derive(Debug, Clone, Default)]
pub(crate) struct ProjectionStats {
    pub session_id: String,
    pub turn_count: usize,
    #[allow(dead_code)]
    pub has_compaction_summary: bool,
    #[allow(dead_code)]
    pub compacted_turns: usize,
    pub checkpoint_count: usize,
    pub checkpoint_covered_turns: usize,
    pub tail_turns: usize,
    pub latest_checkpoint_at: Option<String>,
    pub usage: UsageSnapshot,
}

/// 投影校验警告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectionWarning {
    pub message: String,
}

/// 动态上下文来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicContextSource {
    pub key: String,
    pub chars: usize,
}

/// provider 请求投影视图。
#[derive(Debug, Clone)]
pub(crate) struct ProjectedRequest {
    #[allow(dead_code)]
    pub kind: ProjectionKind,
    pub messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    pub tool_count: usize,
    pub estimate: ProjectionEstimate,
    pub dynamic_sources: Vec<DynamicContextSource>,
    pub warnings: Vec<ProjectionWarning>,
}

/// provider base context 投影视图。
#[derive(Debug, Clone)]
pub(crate) struct ProjectedBaseContext {
    pub messages: Vec<ChatMessage>,
    pub dynamic_sources: Vec<DynamicContextSource>,
}

/// 命令摘要投影视图。
#[derive(Debug, Clone)]
pub(crate) struct ProjectedSessionSummary {
    #[allow(dead_code)]
    pub kind: ProjectionKind,
    pub estimate: ProjectionEstimate,
    pub stats: ProjectionStats,
    pub compaction: Option<CompactionSummary>,
    pub recovery: RecoverySnapshot,
    pub warnings: Vec<ProjectionWarning>,
}
