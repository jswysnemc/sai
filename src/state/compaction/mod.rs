mod budget;
mod estimate;
mod handoff;
mod model;
mod policy;
mod projection_budget;
mod prompt;
mod selector;
mod storage;
mod store;
mod validation;

#[allow(unused_imports)]
pub use budget::RESERVED_CONTEXT_CHARS;
pub use budget::{
    classify_context_pressure_with, should_compact_for_context_tokens_with, CompactionBudgetPolicy,
    ContextPressure,
};
pub use estimate::{estimate_chat_messages_chars, estimate_chat_messages_tokens, occupancy_tokens};
pub use handoff::summary_context_message;
#[allow(unused_imports)]
pub use model::RunningTurnCompaction;
pub use model::{CompactionRequest, CompactionSummary};
pub use policy::ResolvedCompactionPolicy;
pub use selector::select_compaction_with;
#[allow(unused_imports)]
pub(crate) use selector::{PRESERVED_RECENT_TURNS, PRESERVED_RUNNING_TOOL_CALLS};
pub use storage::{clear_summary, load_summary, save_summary};
pub use store::CompactionApplyOutcome;
pub(crate) use validation::{summary_char_limit, validate_summary};
