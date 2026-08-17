mod budget;
mod estimate;
mod handoff;
mod message_origin;
mod model;
mod projection_budget;
mod prompt;
mod selector;
mod storage;
mod store;
mod user_messages;
mod validation;

#[allow(unused_imports)]
pub use budget::RESERVED_CONTEXT_CHARS;
pub use budget::{
    classify_context_pressure, classify_context_pressure_with, compaction_trigger_chars,
    should_compact_for_context_tokens_with, CompactionBudgetPolicy, ContextPressure,
};
pub use estimate::{estimate_chat_messages_chars, estimate_chat_messages_tokens};
pub(in crate::state) use handoff::elision_marker_message;
pub use handoff::summary_context_message;
#[allow(unused_imports)]
pub(crate) use message_origin::is_real_user_input;
#[allow(unused_imports)]
pub use model::RunningTurnCompaction;
pub use model::{CompactionRequest, CompactionSummary};
pub use selector::{select_compaction, select_compaction_with};
#[allow(unused_imports)]
pub(crate) use selector::{PRESERVED_RECENT_TURNS, PRESERVED_RUNNING_TOOL_CALLS};
pub use storage::{clear_summary, load_summary, save_summary};
pub use store::CompactionApplyOutcome;
pub(in crate::state) use user_messages::{
    collect_compactable_user_messages, select_kept_user_messages, KEPT_USER_MESSAGE_HEAD_CHARS,
    KEPT_USER_MESSAGE_MAX_CHARS,
};
pub(crate) use validation::{summary_char_limit, validate_summary};
