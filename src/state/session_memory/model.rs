/// 当前会话工作记忆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionMemory {
    pub session_id: String,
    pub summary: String,
    pub last_summarized_turn_id: Option<String>,
    pub last_summarized_seq: i64,
    pub checkpoint_id: Option<String>,
    pub source_turn_count: usize,
    pub token_estimate: usize,
    pub consecutive_failures: usize,
    pub disabled_until: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 待写入的会话工作记忆。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewSessionMemory {
    pub session_id: String,
    pub summary: String,
    pub last_summarized_turn_id: Option<String>,
    pub last_summarized_seq: i64,
    pub checkpoint_id: Option<String>,
    pub source_turn_count: usize,
    pub token_estimate: usize,
}
