mod agent_presets;
mod agents;
mod app;
mod app_prompts;
mod app_validation;
mod defaults;
mod gateway_defaults;
mod git;
mod mcp_file;
mod model;
mod model_metadata;
mod model_units;
mod notification;
mod paths;
mod permission;
mod provider;
mod secrets;
mod session;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use agent_presets::{ensure_surface_agent_defaults, seed_default_agent_profiles};
#[allow(unused_imports)]
pub use agents::*;
#[allow(unused_imports)]
pub use git::*;
#[allow(unused_imports)]
pub use mcp_file::{init_mcp_config_file, load_mcp_config, save_mcp_config, validate_mcp_config};
pub use model::*;
pub use model_metadata::*;
pub use model_units::*;
pub use notification::*;
pub use permission::*;
#[allow(unused_imports)]
pub use session::SessionConfig;
