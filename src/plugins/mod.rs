mod binary;
mod bundled;
mod compatibility;
mod config;
mod discovery;
mod events;
mod grants;
mod host;
mod http;
mod management;
mod management_lock;
pub(crate) mod operation;
mod presentation;
mod private;
mod registry;
mod services;
mod session;
mod system;

pub(crate) use discovery::{discover, PluginDiagnostic, PluginSource};
pub(crate) use events::PluginEvents;
pub(crate) use grants::{GrantChanges, GrantUpdate};
pub(crate) use management::{configure, install, remove, scaffold, set_enabled, validate_package};
pub(crate) use presentation::notification_plan;
pub(crate) use private::clear_session_storage;
pub(crate) use registry::{register_bundled_catalog_tools, register_plugins};
pub(crate) use services::{PluginModelSource, PluginServices};
pub(crate) use session::PluginSession;

#[cfg(test)]
mod tests;
