mod agent_engine;
mod agent_presets;
mod agent_tool_modes;
mod agents;
mod app;
mod app_prompts;
mod app_validation;
mod cli_tool_defaults;
mod cli_tools;
pub mod defaults;
mod gateway_defaults;
mod git;
mod mcp_file;
mod model;
mod model_metadata;
mod model_units;
mod notification;
mod paths;
mod permission;
mod prompt_templates;
mod provider;
mod provider_keys;
mod secrets;
mod session;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod web_search_tests;

#[allow(unused_imports)]
pub use agent_engine::{AcpEngineConfig, AgentEngineConfig, AgentEngineKind};
#[allow(unused_imports)]
pub use agent_presets::{ensure_surface_agent_defaults, seed_default_agent_profiles};
#[allow(unused_imports)]
pub use agent_tool_modes::{normalize_deferred_tools, DEFERRED_ALL_NON_BASE};
#[allow(unused_imports)]
pub use agents::*;
pub use cli_tools::*;
#[allow(unused_imports)]
pub use git::*;
#[allow(unused_imports)]
pub use mcp_file::{
    init_mcp_config_file, load_mcp_config, parse_mcp_config_value, save_mcp_config,
    validate_mcp_config,
};
pub use model::*;
pub use model_metadata::*;
pub use model_units::*;
pub use notification::*;
pub use permission::*;
pub use prompt_templates::{PromptTemplateConfig, PromptTemplatesConfig};
pub use provider_keys::*;
#[allow(unused_imports)]
pub use session::SessionConfig;
