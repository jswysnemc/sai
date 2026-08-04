mod capture_policy;
mod dedupe;
mod extraction_prompt;
mod llm_extractor;

pub(super) use capture_policy::{evaluate as evaluate_capture, CapturePolicyVerdict};
pub(super) use dedupe::{evaluate as evaluate_dedupe, DedupeOutcome};
pub use llm_extractor::extract_candidates;
