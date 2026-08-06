use super::{AgentMode, ToolVisibility};
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::state::request_projection::DynamicContextSource;
use crate::state::StateStore;
use crate::tools::ToolRegistry;

/// 保存单个会话 Agent 的运行依赖和可变状态。
pub struct Agent {
    pub(super) state: StateStore,
    pub(super) client: OpenAiCompatibleClient,
    pub(super) compaction_client: OpenAiCompatibleClient,
    pub(super) compaction_model_label: String,
    pub(super) base_system_prompt: String,
    /// 上下文窗口 Token 数经保守换算得到的字符预算
    pub(super) context_char_budget: usize,
    pub(super) tools_enabled: bool,
    pub(super) max_tool_rounds: usize,
    pub(super) tools: ToolRegistry,
    pub(super) tool_visibility: ToolVisibility,
    pub(super) memory: MemoryStore,
    pub(super) mode: AgentMode,
    /// 运行中可热更新的模式，供终端快捷键立即切换
    pub(super) live_mode: std::sync::Arc<std::sync::atomic::AtomicU8>,
    pub(super) config: AppConfig,
    pub(super) paths: SaiPaths,
    pub(super) last_dynamic_sources: Vec<DynamicContextSource>,
    /// 外部对话内核；为空时使用内置循环
    pub(super) external_engine: Option<Box<dyn crate::agent_engine::ExternalTurnEngine>>,
}
