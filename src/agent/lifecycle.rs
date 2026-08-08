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
        prepare_session_context(&state, &base_system_prompt)?;
        let context_char_budget = config.active_context_window_tokens()?;
        let compaction_runtime = compaction_model::resolve_compaction_runtime(&config, paths)?;
        let max_tool_rounds = config.tools.max_rounds;
        crate::goal::register_tools(&mut tools, state.goal_file());
        // 渐进加载由当前 Agent 的 deferred_tools 决定；
        // skill 提示词只给名称与简介，正文一律靠 load 读取，因此有可见 skill 时同样注册
        let mut tool_visibility = ToolVisibility::from_config(&config);
        if tools_enabled
            && (tool_visibility.is_progressive()
                || (config.skills.enabled && config.has_visible_skills()))
        {
            tools::register_progressive_loader(&mut tools, config.agent_deferred_tools());
        }
        if tool_visibility.is_progressive() {
            let loaded = state.load_loaded_tools()?;
            tool_visibility.restore_loaded_tools(&tools, &loaded);
        }
        let memory = MemoryStore::new(&config, paths);
        memory.init()?;
        let live_mode = tools
            .permission_mode_handle()
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(mode.as_u8())));
        live_mode.store(mode.as_u8(), std::sync::atomic::Ordering::SeqCst);
        tools.set_permission_mode(mode.permission_profile_mode());
        // 配置指定外部内核时在此构建；构建失败直接报错而不是静默退回原生，
        // 否则用户以为换了内核，实际仍在用 sai 自己的循环
        // 外部内核与自带工具共用同一份权限配置，换内核不会绕过治理
        let governance = crate::acp::AcpGovernance::new(
            crate::runtime_cwd::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            tools.permission_profile(),
            config.clone(),
            state.session_id().to_string(),
            Some(paths),
            Some(state.state_dir()),
        );
        let external_engine = crate::agent_engine::build_external_engine(&config.agent, governance)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
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
            live_mode,
            config,
            paths: paths.clone(),
            last_dynamic_sources: Vec::new(),
            external_engine,
        })
    }

    /// 返回当前 Agent 模式。
    ///
    /// 返回:
    /// - 当前模式
    pub fn mode(&self) -> AgentMode {
        AgentMode::from_u8(self.live_mode.load(std::sync::atomic::Ordering::SeqCst))
    }

    /// 返回当前工具注册表对应的模式。
    ///
    /// 返回:
    /// - 已安装工具注册表使用的模式
    pub fn installed_mode(&self) -> AgentMode {
        self.mode
    }

    /// 返回可在运行期热切换的模式句柄。
    pub fn live_mode_handle(&self) -> std::sync::Arc<std::sync::atomic::AtomicU8> {
        self.live_mode.clone()
    }

    /// 立即应用新模式：更新权限策略；YOLO 时放行全部待审权限。
    ///
    /// 参数:
    /// - `mode`: 新模式
    ///
    /// 返回:
    /// - 无
    #[allow(dead_code)]
    pub fn apply_live_mode(&self, mode: AgentMode) {
        self.live_mode
            .store(mode.as_u8(), std::sync::atomic::Ordering::SeqCst);
        self.tools
            .set_permission_mode(mode.permission_profile_mode());
        // 切到 YOLO：立刻允许所有待审请求，避免继续点「允许一次」
        if mode == AgentMode::Yolo {
            let _ = crate::permission::allow_all_pending_for_session(self.session_id());
        }
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
    /// - 模式切换与会话工具状态恢复结果
    pub fn switch_mode(&mut self, mode: AgentMode, mut tools: ToolRegistry) -> Result<()> {
        let deferred = self.config.agent_deferred_tools();
        let loaded = if deferred.is_empty() {
            Vec::new()
        } else {
            self.state.load_loaded_tools()?
        };
        self.mode = mode;
        crate::goal::register_tools(&mut tools, self.state.goal_file());
        if self.tools_enabled && !deferred.is_empty() {
            tools::register_progressive_loader(&mut tools, deferred);
        }
        self.tools = tools;
        // 与工具权限配置共享原子模式，保证运行中 Shift+Tab 立即生效
        self.live_mode = self
            .tools
            .permission_mode_handle()
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::atomic::AtomicU8::new(mode.as_u8())));
        self.live_mode
            .store(mode.as_u8(), std::sync::atomic::Ordering::SeqCst);
        self.tools
            .set_permission_mode(mode.permission_profile_mode());
        // 1. 按新模式注册表恢复会话已加载集合，不保留当前模式不存在的工具
        self.tool_visibility = ToolVisibility::from_config(&self.config);
        self.tool_visibility
            .restore_loaded_tools(&self.tools, &loaded);
        Ok(())
    }

    /// 替换工具注册表并保留当前模式和渐进加载状态。
    ///
    /// 参数:
    /// - `tools`: 后台发现完成后的完整工具注册表
    ///
    /// 返回:
    /// - 无
    pub fn replace_tools(&mut self, mut tools: ToolRegistry) {
        let loaded = self.tool_visibility.loaded_tool_names();
        let deferred = self.config.agent_deferred_tools();
        crate::goal::register_tools(&mut tools, self.state.goal_file());
        if self.tools_enabled && !deferred.is_empty() {
            tools::register_progressive_loader(&mut tools, deferred);
        }
        self.tools = tools;
        self.tool_visibility
            .restore_loaded_tools(&self.tools, &loaded);
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
        prepare_session_context(&self.state, &self.base_system_prompt)?;
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
        self.live_mode
            .store(mode.as_u8(), std::sync::atomic::Ordering::SeqCst);
        self.tools_enabled =
            self.config.tools.enabled && self.config.active_model_tools_enabled()?;
        crate::goal::register_tools(&mut tools, self.state.goal_file());
        // 与初始化保持一致：有可见 skill 时同样需要加载器读取正文
        if self.tools_enabled
            && (!self.config.agent_deferred_tools().is_empty()
                || (self.config.skills.enabled && self.config.has_visible_skills()))
        {
            tools::register_progressive_loader(&mut tools, self.config.agent_deferred_tools());
        }
        self.tools = tools;
        self.tool_visibility = ToolVisibility::from_config(&self.config);
        if self.tool_visibility.is_progressive() {
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
        crate::goal::register_tools(&mut self.tools, self.state.goal_file());
        self.tool_visibility = ToolVisibility::from_config(&self.config);
        if self.tool_visibility.is_progressive() {
            let loaded = self.state.load_loaded_tools()?;
            self.restore_loaded_tools(&loaded);
        }
        self.last_dynamic_sources.clear();
        prepare_session_context(&self.state, &self.base_system_prompt)?;
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

/// 同步当前模式的稳定系统提示并恢复过期轮次。
///
/// 参数:
/// - `state`: 当前会话状态
/// - `base_system_prompt`: 不含模式说明的基础系统提示
///
/// 返回:
/// - 上下文同步与恢复结果
fn prepare_session_context(state: &StateStore, base_system_prompt: &str) -> Result<()> {
    state.reset_if_prompt_changed(base_system_prompt)?;
    state.recover_stale_turns()?;
    Ok(())
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
