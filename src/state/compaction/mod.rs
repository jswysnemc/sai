mod budget;
mod estimate;
mod model;
mod projection_budget;
mod prompt;
mod selector;
mod storage;
mod store;
mod validation;

pub use budget::{
    classify_context_pressure, compaction_trigger_chars, should_compact_for_context_tokens,
    ContextPressure,
};
pub use estimate::{estimate_chat_messages_chars, estimate_chat_messages_tokens};
pub use model::{CompactionRequest, CompactionSummary};
pub use prompt::summary_context_message;
pub use selector::select_compaction;
pub(crate) use selector::PRESERVED_RECENT_TURNS;
pub use storage::{clear_summary, load_summary, save_summary};
pub use store::CompactionApplyOutcome;
pub(crate) use validation::{summary_char_limit, validate_summary};
