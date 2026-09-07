mod bundled;
mod config;
mod discovery;
mod events;
mod host;
mod management;
mod registry;
mod session;

pub(crate) use discovery::{discover, PluginDiagnostic, PluginSource};
pub(crate) use events::PluginEvents;
pub(crate) use management::{
    configure, install, remove, scaffold, set_enabled, validate_package, GrantUpdate,
};
pub(crate) use registry::{register_bundled_catalog_tools, register_plugins};
pub(crate) use session::PluginSession;

#[cfg(test)]
mod tests;
