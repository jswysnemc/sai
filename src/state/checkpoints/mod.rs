pub(in crate::state) mod schema;

mod migration;
mod model;
mod projection;
mod repository;

pub(in crate::state) use migration::migrate_legacy_compaction_summary;
#[allow(unused_imports)]
pub(crate) use model::{CheckpointReason, CheckpointStats, CompactionCheckpoint, ProjectedHistory};
#[allow(unused_imports)]
pub(in crate::state) use projection::{project_history, project_history_from_parts};
pub(in crate::state) use repository::{
    apply_checkpoint_compaction, count_checkpoints, load_latest_checkpoint,
};
