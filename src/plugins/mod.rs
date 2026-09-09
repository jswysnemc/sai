mod bundled;
mod compatibility;
mod config;
mod discovery;
mod events;
mod grants;
mod host;
mod http;
mod management;
mod registry;
mod services;
mod session;

pub(crate) use discovery::{discover, PluginDiagnostic, PluginSource};
pub(crate) use events::PluginEvents;
pub(crate) use grants::{GrantChanges, GrantUpdate};
pub(crate) use management::{configure, install, remove, scaffold, set_enabled, validate_package};
pub(crate) use registry::{register_bundled_catalog_tools, register_plugins};
pub(crate) use services::{PluginModelSource, PluginServices};
pub(crate) use session::PluginSession;

#[cfg(test)]
mod tests;
