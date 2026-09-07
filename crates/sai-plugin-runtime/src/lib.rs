#![forbid(unsafe_code)]

mod contracts;
pub mod host;
mod manifest;
mod package;
mod runtime;

pub use contracts::{EventContext, EventKind, PluginCommand, PluginTool, ToolAccess};
pub use manifest::{Capabilities, ExecutionLimits, PluginManifest, API_VERSION};
pub use package::PluginPackage;
pub use runtime::{InvocationContext, PluginRuntime, ProgressCallback};
