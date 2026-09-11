#![deny(unsafe_op_in_unsafe_fn)]

mod allocation;
mod snapshot;

pub use snapshot::MemoryDatabase;
