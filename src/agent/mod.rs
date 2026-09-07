mod agent_state;
mod compaction;
mod compaction_model;
mod compaction_replay;
pub(crate) mod compaction_schema;
mod context_projection;
mod context_resources;
mod conversation;
mod deepseek_anchor;
mod edit_guard;
mod event;
mod external_events;
mod external_tool_history;
mod external_turn;
mod instruction_files;
mod inter_message;
mod lifecycle;
mod load_request;
mod message_context;
mod message_gap;
mod message_request;
mod message_usage;
mod mode;
mod model_context;
pub(crate) mod model_json;
mod plugin_commands;
mod recovery;
pub(crate) mod repeat_guard;
mod runtime_context;
mod skill_load;
pub(crate) mod system_prompt;
mod tool_attachments;
mod tool_batch_execution;
mod tool_execution;
mod tool_gate;
mod tool_history;
mod tool_invocation;
mod tool_visibility;
mod turn_execution;
mod turn_orchestration;
mod turn_settlement;
mod turn_tools;

use crate::config::AppConfig;
use crate::llm::{ChatMessage, ChatResult, OpenAiCompatibleClient};
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::{
    project_provider_base_context_projection, project_provider_turn_from_base_projection,
    project_provider_turn_from_messages, DynamicContextSource, ProjectedBaseContext,
};
use crate::state::StateStore;
use crate::tools::{self, memes, ToolPermission, ToolRegistry};
use anyhow::Result;
use message_context::system_messages_first;
use model_context::selected_model_label;
pub(crate) use runtime_context::{context_state_update, RuntimeContextSnapshot};
pub(crate) use tool_gate::{evaluate_tool_gate, ToolGate};
use tool_gate::{is_tool_error_output, tool_error_output};
pub(crate) use tool_visibility::ToolVisibility;
use turn_execution::assistant_tool_message;

pub use agent_state::Agent;
pub(crate) use compaction::CompactionRunOutcome;
pub(crate) use context_resources::{
    combine_context_updates, context_resource_update, context_resource_update_against_baseline,
};
pub use event::{AgentEvent, CompactionError, MessageContextUpdate};
pub(crate) use external_events::{ExternalEventBatch, ExternalEventMonitor, ExternalEventWake};
pub(crate) use instruction_files::{extract_instruction_files, load_instruction_prompt};
pub(crate) use inter_message::{
    InterMessage, InterMessageEvent, InterMessageKind, InterMessageSource,
};
pub use mode::AgentMode;
pub(crate) use model_json::first_json_object;
pub(crate) use system_prompt::{build_base_system_prompt, build_base_system_prompt_for_phase};
pub(crate) use tool_invocation::resolve_execution_call;
