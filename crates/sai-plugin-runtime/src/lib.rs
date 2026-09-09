#![forbid(unsafe_code)]

mod capabilities;
mod contracts;
pub mod host;
mod manifest;
mod package;
mod runtime;
mod schema;

pub use capabilities::{
    Capabilities, ProcessArgument, ProcessParameter, ProcessTemplate, SystemCapabilities,
};
pub use contracts::{EventContext, EventKind, PluginCommand, PluginTool, ToolAccess};
pub use manifest::{ExecutionLimits, PluginManifest, API_VERSION};
pub use package::PluginPackage;
pub use runtime::{InvocationContext, PluginRuntime, ProgressCallback};
