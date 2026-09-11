#![forbid(unsafe_code)]

mod capabilities;
mod contracts;
pub mod host;
mod manifest;
mod package;
mod presentation;
mod reply_policy;
mod runtime;
mod schema;
mod sqlite;
mod tool_policy;

pub use capabilities::{
    BinaryCapabilities, Capabilities, ProcessArgument, ProcessParameter, ProcessTemplate,
    SystemCapabilities,
};
pub use contracts::{EventContext, EventKind, PluginCommand, PluginTool, ToolAccess};
pub use manifest::{ExecutionLimits, PluginManifest, API_VERSION};
pub use package::PluginPackage;
pub use presentation::{
    Notification, PresentationRuntime, PresentationSurface, ReplyPresentation, ReplyStatus,
    MAX_NOTIFICATIONS,
};
pub use reply_policy::{
    PreparedReply, MAX_REPLY_CONTEXT_BYTES, MAX_REPLY_DELIVERY_BYTES, MAX_REPLY_INPUT_BYTES,
};
pub use runtime::{InvocationContext, PluginRuntime, ProgressCallback};
pub use tool_policy::{
    ToolPolicyInput, ToolPolicyOutput, MAX_TOOL_POLICY_INPUT_BYTES, MAX_TOOL_POLICY_STATE_BYTES,
};
