use super::*;

impl Agent {
    /// 创建使用指定配置、状态和工具注册表的 Agent。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `paths`: Sai 路径
    /// - `state`: 状态存储
    /// - `client`: LLM 客户端
    /// - `tools`: 工具注册表
    /// - `mode`: Agent 模式
    ///
    /// 返回:
    /// - 初始化完成的 Agent
    pub fn new(
        config: AppConfig,
        paths: &SaiPaths,
        state: StateStore,
        client: OpenAiCompatibleClient,
        tools: ToolRegistry,
        mode: AgentMode,
    ) -> Result<Self> {
        Self::new_with_extra_system_prompt(config, paths, state, client, tools, mode, None)
    }

    /// 创建带额外系统提示词的 Agent。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `paths`: Sai 路径
    /// - `state`: 状态存储
    /// - `client`: LLM 客户端
    /// - `tools`: 工具注册表
    /// - `mode`: Agent 模式
    /// - `extra_system_prompt`: 额外系统提示词
    ///
    /// 返回:
    /// - Agent 实例
    pub fn new_with_extra_system_prompt(
        config: AppConfig,
        paths: &SaiPaths,
        state: StateStore,
        client: OpenAiCompatibleClient,
        mut tools: ToolRegistry,
        mode: AgentMode,
        extra_system_prompt: Option<&str>,
    ) -> Result<Self> {
        let tools_enabled = config.tools.enabled && config.active_model_tools_enabled()?;
        let base_system_prompt =
            build_base_system_prompt(&config, paths, tools_enabled, extra_system_prompt)?;
        if mode != AgentMode::Plan {
            state.reset_if_prompt_changed(&base_system_prompt)?;
            state.recover_stale_turns()?;
        }
        let context_char_budget = config.active_context_window_tokens()?;
        let compaction_runtime = compaction_model::resolve_compaction_runtime(&config, paths)?;
        let max_tool_rounds = config.tools.max_rounds;
        if tools_enabled && config.tools.progressive_loading_enabled {
            tools::register_progressive_loader(&mut tools);
        }
        let tool_visibility = ToolVisibility::new(config.tools.progressive_loading_enabled);
        let memory = MemoryStore::new(&config, paths);
        memory.init()?;
        Ok(Self {
            state,
            client,
            compaction_client: compaction_runtime.client,
            compaction_model_label: compaction_runtime.label,
            base_system_prompt,
            context_char_budget,
            tools_enabled,
            max_tool_rounds,
            tools,
            tool_visibility,
            memory,
            mode,
            config,
            paths: paths.clone(),
            last_dynamic_sources: Vec::new(),
        })
    }

    /// 返回当前 Agent 模式。
    ///
    /// 返回:
    /// - 当前模式
    pub fn mode(&self) -> AgentMode {
        self.mode
    }

    /// 返回当前会话 ID。
    ///
    /// 返回:
    /// - 会话标识
    pub fn session_id(&self) -> &str {
        self.state.session_id()
    }

    /// 返回状态存储引用（runner 持久化渐进工具等）。
    ///
    /// 返回:
    /// - 状态存储
    pub fn state(&self) -> &StateStore {
        &self.state
    }

    /// 切换模式并替换工具注册表（REPL 复用 Agent 时使用）。
    ///
    /// 参数:
    /// - `mode`: 新模式
    /// - `tools`: 与模式匹配的工具注册表
    ///
    /// 返回:
    /// - 无
    pub fn switch_mode(&mut self, mode: AgentMode, mut tools: ToolRegistry) {
        self.mode = mode;
        if self.tools_enabled && self.config.tools.progressive_loading_enabled {
            tools::register_progressive_loader(&mut tools);
        }
        self.tools = tools;
        // 1. 重置渐进加载可见性，避免跨模式残留
        self.tool_visibility = ToolVisibility::new(self.config.tools.progressive_loading_enabled);
    }

    /// 每轮对话前轻量刷新：系统提示、上下文窗口、过期 turn 恢复。
    ///
    /// 返回:
    /// - 刷新是否成功
    pub fn prepare_for_turn(&mut self) -> Result<()> {
        self.tools_enabled =
            self.config.tools.enabled && self.config.active_model_tools_enabled()?;
        self.base_system_prompt =
            build_base_system_prompt(&self.config, &self.paths, self.tools_enabled, None)?;
        if self.mode != AgentMode::Plan {
            self.state
                .reset_if_prompt_changed(&self.base_system_prompt)?;
            self.state.recover_stale_turns()?;
        }
        self.context_char_budget = self.config.active_context_window_tokens()?;
        self.max_tool_rounds = self.config.tools.max_rounds;
        Ok(())
    }

    /// 配置/客户端变更后全量重载（providers、模型、thinking 等）。
    ///
    /// 参数:
    /// - `config`: 新配置
    /// - `client`: 新 LLM 客户端
    /// - `tools`: 新工具注册表
    /// - `mode`: 当前模式
    ///
    /// 返回:
    /// - 重载是否成功
    pub fn reload(
        &mut self,
        config: AppConfig,
        client: OpenAiCompatibleClient,
        mut tools: ToolRegistry,
        mode: AgentMode,
    ) -> Result<()> {
        let compaction_runtime =
            compaction_model::resolve_compaction_runtime(&config, &self.paths)?;
        self.config = config;
        self.client = client;
        self.compaction_client = compaction_runtime.client;
        self.compaction_model_label = compaction_runtime.label;
        self.mode = mode;
        self.tools_enabled =
            self.config.tools.enabled && self.config.active_model_tools_enabled()?;
        if self.tools_enabled && self.config.tools.progressive_loading_enabled {
            tools::register_progressive_loader(&mut tools);
        }
        self.tools = tools;
        self.tool_visibility = ToolVisibility::new(self.config.tools.progressive_loading_enabled);
        if self.config.tools.progressive_loading_enabled {
            let loaded = self.state.load_loaded_tools()?;
            self.restore_loaded_tools(&loaded);
        }
        self.memory = MemoryStore::new(&self.config, &self.paths);
        self.memory.init()?;
        self.prepare_for_turn()
    }

    /// 切换会话状态（/new、/resume、/clear 后）。
    ///
    /// 参数:
    /// - `state`: 新状态存储
    ///
    /// 返回:
    /// - 切换是否成功
    pub fn replace_state(&mut self, state: StateStore) -> Result<()> {
        self.state = state;
        self.tool_visibility = ToolVisibility::new(self.config.tools.progressive_loading_enabled);
        if self.config.tools.progressive_loading_enabled {
            let loaded = self.state.load_loaded_tools()?;
            self.restore_loaded_tools(&loaded);
        }
        self.last_dynamic_sources.clear();
        if self.mode != AgentMode::Plan {
            self.state
                .reset_if_prompt_changed(&self.base_system_prompt)?;
            self.state.recover_stale_turns()?;
        }
        Ok(())
    }

    /// 重建记忆存储（/clear all 后）。
    ///
    /// 返回:
    /// - 重建是否成功
    pub fn reset_memory(&mut self) -> Result<()> {
        self.memory = MemoryStore::new(&self.config, &self.paths);
        self.memory.init()?;
        Ok(())
    }

    /// 恢复上一轮已经加载的工具集合。
    ///
    /// 参数:
    /// - `loaded_tools`: 上一轮保存的已加载工具名称
    ///
    /// 返回:
    /// - 无
    pub fn restore_loaded_tools(&mut self, loaded_tools: &[String]) {
        self.tool_visibility
            .restore_loaded_tools(&self.tools, loaded_tools);
    }

    /// 导出当前已经加载的工具集合。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前已经加载的工具名称列表
    pub fn loaded_tools(&self) -> Vec<String> {
        self.tool_visibility.loaded_tool_names()
    }

    /// 导出最近一次 provider 请求的动态上下文来源。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 动态上下文来源列表
    pub fn last_dynamic_sources(&self) -> Vec<DynamicContextSource> {
        self.last_dynamic_sources.clone()
    }
}
