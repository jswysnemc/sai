#![forbid(unsafe_code)]

mod capabilities;
mod contracts;
pub mod host;
mod manifest;
mod package;
mod runtime;

pub use capabilities::Capabilities;
pub use contracts::{EventContext, EventKind, PluginCommand, PluginTool, ToolAccess};
pub use manifest::{ExecutionLimits, PluginManifest, API_VERSION};
pub use package::PluginPackage;
pub use runtime::{InvocationContext, PluginRuntime, ProgressCallback};
