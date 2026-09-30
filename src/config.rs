mod agent_engine;
mod agent_presets;
mod agent_tool_modes;
mod agents;
mod app;
mod app_config;
mod app_prompts;
mod app_validation;
mod cli_tool_defaults;
mod cli_tools;
pub mod defaults;
mod gateway_defaults;
mod git;
mod jev;
mod jev_legacy;
mod mcp_file;
mod mesh;
mod model;
mod model_endpoints;
mod model_metadata;
mod model_thinking;
mod model_units;
mod paths;
mod permission;
mod prompt_sections;
mod prompt_templates;
mod provider;
mod provider_choices;
mod provider_keys;
mod sandbox;
mod secrets;
mod session;
mod ssh;
mod subagent_models;
mod tool_whitelist;
mod web_search;

pub use web_search::WebSearchConfig;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use agent_engine::{AcpEngineConfig, AgentEngineConfig, AgentEngineKind};
#[allow(unused_imports)]
pub use agent_presets::{ensure_surface_agent_defaults, seed_default_agent_profiles};
#[allow(unused_imports)]
pub use agent_tool_modes::{normalize_deferred_tools, DEFERRED_ALL_NON_BASE};
#[allow(unused_imports)]
pub use agents::*;
#[allow(unused_imports)]
pub use cli_tools::*;
#[allow(unused_imports)]
pub use git::*;
#[allow(unused_imports)]
pub use jev::{
    jev_connection_for, jev_connection_info_for, JevAuditConfig, JevConfig, JevConnection,
    JevConnectionInfo, JevConnectionSource, JevRoutingConfig, JEV_DEFAULT_MODEL, JEV_KEY_ENV_NAMES,
    JEV_OFFICIAL_ENDPOINT,
};
#[allow(unused_imports)]
pub use mcp_file::{
    init_mcp_config_file, load_mcp_config, parse_mcp_config_value, save_mcp_config,
    validate_mcp_config,
};
pub use model::*;
pub use model_endpoints::{ModelEndpointConfig, ModelEndpointKind};
pub use model_metadata::*;
pub use model_thinking::*;
pub use model_units::*;
pub use permission::*;
pub use prompt_sections::{PromptSectionToggles, PROMPT_SECTIONS};
pub use prompt_templates::{PromptTemplateConfig, PromptTemplatesConfig};
pub use provider_keys::*;
pub use sandbox::{SandboxConfig, SandboxNetworkMode};
#[allow(unused_imports)]
pub use session::SessionConfig;
#[allow(unused_imports)]
pub use ssh::{SshConfig, SshHostConfig, DEFAULT_SSH_PORT};
pub(crate) use subagent_models::{
    subagent_runtime_config, SubagentModelChoice, SubagentModelSettings,
};
pub use tool_whitelist::{unknown_whitelist_tools, whitelist_allows_tool};
