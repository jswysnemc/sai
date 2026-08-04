mod capture;
mod injection;
mod library;
mod model;
mod persistence;
mod retrieval;
mod store_maintenance;
mod store_types;

pub use capture::extract_candidates;
pub use library::{now_days, MemoryLibrary};
pub use model::{MemoryCandidate, MemoryKind, MemoryScope};
pub use store_types::{AssociationContext, EvictedTurn, MemoryHit};

include!("store.rs");
include!("stats.rs");
include!("storage.rs");
include!("tests.rs");
