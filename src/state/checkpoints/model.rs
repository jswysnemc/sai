use crate::llm::ChatMessage;
use crate::state::StoredConversationEntry;
use serde::{Deserialize, Serialize};

/// checkpoint 写入原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum CheckpointReason {
    Auto,
    Manual,
    Legacy,
}

/// 会话压缩 checkpoint。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactionCheckpoint {
    pub id: String,
    pub seq: i64,
    pub compacted_from_seq: i64,
    pub compacted_to_seq: i64,
    pub summary: String,
    pub recent: String,
    pub source_turn_count: usize,
    pub reason: CheckpointReason,
    pub created_at: String,
}

/// checkpoint 投影统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CheckpointStats {
    pub checkpoint_count: usize,
    pub covered_turns: usize,
    pub tail_turns: usize,
    pub latest_checkpoint_at: Option<String>,
}

/// 会话历史投影。
#[derive(Debug, Clone)]
pub(crate) struct ProjectedHistory {
    pub checkpoint_context: Option<String>,
    #[allow(dead_code)]
    pub entries: Vec<StoredConversationEntry>,
    pub messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    pub stats: CheckpointStats,
}
