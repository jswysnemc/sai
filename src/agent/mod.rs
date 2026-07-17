mod compaction;
mod compaction_model;
mod conversation;
mod event;
mod message_context;
mod mode;
mod model_context;
mod recovery;
mod subagent_completion;
mod system_prompt;
mod tool_history;
mod tool_visibility;
mod turn_orchestration;

use crate::config::AppConfig;
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamEvent, ChatStreamKind,
    OpenAiCompatibleClient,
};
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::{
    project_provider_base_context_projection, project_provider_turn_from_base_projection,
    project_provider_turn_from_messages, DynamicContextSource, ProjectedBaseContext,
};
use crate::state::StateStore;
use crate::tools::{self, memes, ToolPermission, ToolRegistry};
use anyhow::{bail, Result};
use message_context::{runtime_context_message, system_messages_first};
use model_context::selected_model_label;
use system_prompt::build_base_system_prompt;
use tokio::sync::mpsc;
use tool_history::extract_persistable_tool_report;
use tool_visibility::ToolVisibility;

pub use event::{AgentEvent, CompactionError};
pub use mode::AgentMode;
pub(crate) use compaction::CompactionRunOutcome;

const MAX_QUESTION_ROUNDS_PER_TURN: usize = 8;
pub struct Agent {
    state: StateStore,
    client: OpenAiCompatibleClient,
    compaction_client: OpenAiCompatibleClient,
    compaction_model_label: String,
    base_system_prompt: String,
    // 上下文窗口 token 数经保守换算得到的字符预算，压缩触发与预算判断均用字符口径
    context_char_budget: usize,
    tools_enabled: bool,
    max_tool_rounds: usize,
    tools: ToolRegistry,
    tool_visibility: ToolVisibility,
    memory: MemoryStore,
    mode: AgentMode,
    config: AppConfig,
    paths: SaiPaths,
    last_dynamic_sources: Vec<DynamicContextSource>,
}

impl Agent {
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

    async fn chat_with_tools<F>(
        &mut self,
        turn_id: &str,
        messages: &mut Vec<ChatMessage>,
        used_tools: &mut Vec<String>,
        persisted_tool_reports: &mut Vec<(String, String)>,
        input: &str,
        image_urls: &[String],
        association_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let mut tool_round = 0usize;
        let mut tool_event_seq = self.state.tool_call_count_for_turn(turn_id)?;
        let mut todo_reminder = self
            .tools
            .contains("todo")
            .then(|| tools::todo::TodoReminder::new(self.state.todo_file()));
        let mut subagent_reminder = self
            .tools
            .contains("subagent")
            .then(|| tools::SubagentReminder::new(self.state.state_dir().display().to_string()));
        let mut question_rounds = 0usize;
        let hook_ctx = crate::hooks::HookContext {
            session_id: self.state.session_id().to_string(),
            workdir: crate::runtime_cwd::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            tool_name: None,
            extra: Default::default(),
        };
        crate::hooks::dispatch(&self.config.hooks, crate::hooks::HookEvent::AgentStart, &hook_ctx).await;
        // 1. 上一轮结束后才完成的子智能体,在本轮首次请求前先补一次通知
        if let Some(reminder) = subagent_reminder.as_mut() {
            if let Some(content) = reminder.after_tool_round() {
                messages.push(ChatMessage::system(content));
            }
        }
        loop {
            if self.max_tool_rounds > 0 && tool_round >= self.max_tool_rounds {
                let content = format!(
                    "工具调用已达到上限 {} 轮，已停止继续调用。可将 `tools.max_rounds` 设为 0 以允许无限工具调用。",
                    self.max_tool_rounds
                );
                on_event(AgentEvent::Chunk(ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: content.clone(),
                }))?;
                crate::hooks::dispatch(&self.config.hooks, crate::hooks::HookEvent::AgentEnd, &hook_ctx).await;
                return Ok(ChatResult {
                    content,
                    reasoning: None,
                    usage: None,
                    tool_calls: Vec::new(),
                });
            }
            tool_round += 1;
            crate::hooks::dispatch(&self.config.hooks, crate::hooks::HookEvent::TurnStart, &hook_ctx).await;
            crate::hooks::dispatch(&self.config.hooks, crate::hooks::HookEvent::MessageStart, &hook_ctx).await;
            self.compact_between_tool_rounds(
                tool_round,
                turn_id,
                messages,
                input,
                image_urls,
                association_prompt,
                auto_meme_reminder,
                on_event,
                perf,
            )
            .await?;
            let definitions = if self.tools_enabled {
                self.tool_visibility.definitions(&self.tools)
            } else {
                Vec::new()
            };
            perf.mark(&format!("round {tool_round} tool definitions"));
            let ordered_messages = system_messages_first(messages.clone());
            let projection = project_provider_turn_from_messages(
                &ordered_messages,
                definitions.len(),
                self.context_char_budget,
            );
            self.state
                .enforce_provider_projection(Some(turn_id), &projection)?;
            perf.mark(&format!("round {tool_round} provider projection"));
            let mut saw_reasoning = false;
            let mut saw_content = false;
            let mut saw_tool_progress = false;
            perf.mark(&format!("round {tool_round} model request start"));
            let result = self
                .client
                .chat_stream_events(ordered_messages, definitions.clone(), |event| match event {
                    ChatStreamEvent::Chunk(chunk) => {
                        match chunk.kind {
                            ChatStreamKind::Reasoning if !saw_reasoning => {
                                saw_reasoning = true;
                                perf.mark(&format!("round {tool_round} first reasoning chunk"));
                            }
                            ChatStreamKind::Content if !saw_content => {
                                saw_content = true;
                                perf.mark(&format!("round {tool_round} first content chunk"));
                            }
                            _ => {}
                        }
                        on_event(AgentEvent::Chunk(chunk))
                    }
                    ChatStreamEvent::ToolCallProgress(progress) => {
                        if !saw_tool_progress {
                            saw_tool_progress = true;
                            perf.mark(&format!("round {tool_round} first tool args chunk"));
                        }
                        on_event(AgentEvent::ToolCallProgress(progress))
                    }
                })
                .await?;
            if let Some(reminder) = subagent_reminder.as_mut() {
                reminder.acknowledge_delivered();
            }
            perf.mark(&format!("round {tool_round} model request done"));
            if result.tool_calls.is_empty() || !self.tools_enabled {
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::MessageEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::TurnEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::AgentEnd,
                    &hook_ctx,
                )
                .await;
                return Ok(result);
            }
            messages.push(ChatMessage::assistant(
                result.content.clone(),
                Some(result.tool_calls.clone()),
            ));
            let ask_question_enabled = self.tools.contains("ask_question");
            let question_call_count = result
                .tool_calls
                .iter()
                .filter(|call| ask_question_enabled && call.function.name == "ask_question")
                .count();
            if question_call_count == 1 {
                question_rounds += 1;
            }
            let question_round_allowed =
                question_call_count == 1 && question_rounds <= MAX_QUESTION_ROUNDS_PER_TURN;
            let defer_sibling_tools = question_call_count == 1 && result.tool_calls.len() > 1;
            for call in result.tool_calls {
                tool_event_seq += 1;
                self.record_tool_call_started(turn_id, tool_event_seq, &call)?;
                used_tools.push(call.function.name.clone());
                perf.mark(&format!("tool {} call recorded", call.function.name));
                on_event(AgentEvent::ToolCall {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                })?;
                if ask_question_enabled && call.function.name == "ask_question" {
                    if question_call_count > 1 {
                        let output = "tool error: only one ask_question call is allowed per tool batch; combine all questions into one call".to_string();
                        self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    if !question_round_allowed {
                        let output = format!(
                            "tool error: ask_question exceeded the per-turn limit of {MAX_QUESTION_ROUNDS_PER_TURN}"
                        );
                        self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    let request = match crate::question::QuestionRequest::parse(&call.function.arguments)
                    {
                        Ok(request) => request,
                        Err(err) => {
                            let output = format!("tool error: invalid ask_question request: {err}");
                            self.record_tool_result_completed(
                                turn_id, &call, false, &output, &output,
                            )?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                    };
                    let (pending, response_rx) =
                        crate::question::request_question(self.session_id(), request.clone());
                    let request_id = pending.id.clone();
                    on_event(AgentEvent::QuestionRequested(pending))?;
                    let response = response_rx
                        .await
                        .unwrap_or(crate::question::QuestionResponse::Cancelled);
                    on_event(AgentEvent::QuestionResolved {
                        request_id,
                        response: response.clone(),
                    })?;
                    let output = match response {
                        crate::question::QuestionResponse::Answered(answers) => {
                            match crate::question::QuestionExchange::new(request, answers) {
                                Ok(exchange) => crate::question::answered_tool_output(&exchange),
                                Err(err) => {
                                    let output = format!("tool error: invalid ask_question answers: {err}");
                                    self.record_tool_result_completed(
                                        turn_id, &call, false, &output, &output,
                                    )?;
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: false,
                                        output: output.clone(),
                                    })?;
                                    messages.push(ChatMessage::tool(call.id, output));
                                    continue;
                                }
                            }
                        }
                        crate::question::QuestionResponse::Cancelled => {
                            let output = crate::question::unavailable_tool_output(
                                "user cancelled the question",
                            );
                            self.record_tool_result_completed(
                                turn_id, &call, false, &output, &output,
                            )?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                        crate::question::QuestionResponse::Unavailable(reason) => {
                            crate::question::unavailable_tool_output(&reason)
                        }
                    };
                    self.record_tool_result_completed(turn_id, &call, true, &output, &output)?;
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: true,
                        output: output.clone(),
                    })?;
                    messages.push(ChatMessage::tool(call.id, output));
                    continue;
                }
                if defer_sibling_tools {
                    let output = "tool error: deferred until the user answers ask_question; reissue this tool call after receiving the answer".to_string();
                    self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    messages.push(ChatMessage::tool(call.id, output));
                    continue;
                }
                if self.mode == AgentMode::Plan
                    && self.tools.permission(&call.function.name)? != ToolPermission::ReadOnly
                {
                    let output = format!(
                        "tool error: Plan mode blocked non-read-only tool: {}",
                        call.function.name
                    );
                    self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                    bail!(
                        "Plan mode blocked non-read-only tool: {}",
                        call.function.name
                    );
                }
                if self
                    .tools
                    .requires_permission(&call.function.name, &call.function.arguments)?
                {
                    self.tools.record_permission_requested(
                        &call.function.name,
                        &call.function.arguments,
                    )?;
                    let (request, decision) = crate::permission::request_permission(
                        self.session_id(),
                        &call.function.name,
                        &call.function.arguments,
                    );
                    let request_id = request.id.clone();
                    on_event(AgentEvent::PermissionRequested(request))?;
                    let decision = decision.await?;
                    on_event(AgentEvent::PermissionResolved {
                        request_id,
                        decision: decision.clone(),
                    })?;
                    match decision {
                        crate::permission::PermissionDecision::Allow => {
                            self.tools.record_permission_approved(
                                &call.function.name,
                                &call.function.arguments,
                            )?;
                        }
                        crate::permission::PermissionDecision::Deny { reply } => {
                            self.tools.record_permission_denied(
                                &call.function.name,
                                &call.function.arguments,
                                reply.as_deref(),
                            )?;
                            let output = reply
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or_else(|| "用户拒绝了此工具调用".to_string());
                            self.record_tool_result_completed(
                                turn_id, &call, false, &output, &output,
                            )?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                    }
                }
                if self.tool_visibility.is_loader_call(&call.function.name) {
                    let output = match self.tool_visibility.load_from_arguments(
                        &self.tools,
                        &call.function.arguments,
                        &self.config,
                        &self.paths,
                    ) {
                        Ok(output) => {
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: true,
                                output: output.clone(),
                            })?;
                            output
                        }
                        Err(err) => {
                            let output = format!("tool error: {err}");
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            output
                        }
                    };
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        !context_output.starts_with("tool error:"),
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                if !self.tool_visibility.is_visible(&call.function.name) {
                    let output = format!(
                        "tool error: tool {} is not loaded in the current visible tool set; call load with tool_name or group_name first. If this tool was loaded in a previous conversation, the loaded-tool session state was reset or is unavailable.",
                        call.function.name
                    );
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        false,
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                if call.function.name == "install_aur_package"
                    && used_tools.iter().any(|name| name == "review_aur_package")
                {
                    let output = "tool error: install_aur_package cannot run in the same turn as review_aur_package. This is a workflow confirmation error, not a tool loading error. Do not call load again; ask the user to confirm installation in a new turn first.".to_string();
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        false,
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
                perf.mark(&format!("tool {} start", call.function.name));
                let mut tool_hook_ctx = hook_ctx.clone();
                tool_hook_ctx.tool_name = Some(call.function.name.clone());
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::ToolExecutionStart,
                    &tool_hook_ctx,
                )
                .await;
                let tool_future = self.tools.call_with_progress(
                    &call.function.name,
                    &call.function.arguments,
                    progress_tx,
                );
                tokio::pin!(tool_future);
                let output = loop {
                    tokio::select! {
                        result = &mut tool_future => {
                            break match result {
                                Ok(output) => {
                                    while let Ok(message) = progress_rx.try_recv() {
                                        on_event(AgentEvent::ToolProgress {
                                            name: call.function.name.clone(),
                                            message,
                                        })?;
                                    }
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: true,
                                        output: output.clone(),
                                    })?;
                                    perf.mark(&format!("tool {} result event", call.function.name));
                                    if let Some(report) = extract_persistable_tool_report(
                                        &call.function.name,
                                        &output,
                                    ) {
                                        persisted_tool_reports
                                            .push((call.function.name.clone(), report));
                                    }
                                    output
                                }
                                Err(err) => {
                                    while let Ok(message) = progress_rx.try_recv() {
                                        on_event(AgentEvent::ToolProgress {
                                            name: call.function.name.clone(),
                                            message,
                                        })?;
                                    }
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: false,
                                        output: format!("tool error: {err}"),
                                    })?;
                                    perf.mark(&format!("tool {} error event", call.function.name));
                                    format!("tool error: {err}")
                                }
                            };
                        }
                        Some(message) = progress_rx.recv() => {
                            on_event(AgentEvent::ToolProgress {
                                name: call.function.name.clone(),
                                message,
                            })?;
                        }
                    }
                };
                let context_output = tools::tool_output_for_context(&call.function.name, &output);
                self.record_tool_result_completed(
                    turn_id,
                    &call,
                    !output.starts_with("tool error:"),
                    &output,
                    &context_output,
                )?;
                perf.mark(&format!("tool {} result persisted", call.function.name));
                messages.push(ChatMessage::tool(call.id, context_output));
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::ToolExecutionEnd,
                    &tool_hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::MessageEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::TurnEnd,
                    &hook_ctx,
                )
                .await;
                if let Some(reminder) = todo_reminder.as_mut() {
                    let todo_updated = call.function.name == "todo"
                        && !output.starts_with("tool error:")
                        && tools::todo::is_mutating_call(&call.function.arguments);
                    if let Some(content) = reminder.after_tool_round(todo_updated)? {
                        messages.push(ChatMessage::system(content));
                    }
                }
            }
            // 1. 本轮所有工具处理完后,把新完成的后台子智能体主动通知主 Agent
            if let Some(reminder) = subagent_reminder.as_mut() {
                if let Some(content) = reminder.after_tool_round() {
                    messages.push(ChatMessage::system(content));
                }
            }
        }
    }

    /// 构造当前轮完整请求消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前运行中轮次标识
    /// - `input`: 当前用户输入
    /// - `image_urls`: 图片 data URL 列表
    /// - `association_prompt`: 可选关联记忆上下文
    /// - `auto_meme_reminder`: 可选自动表情包提醒
    ///
    /// 返回:
    /// - 当前轮请求消息列表
    fn chat_messages_for_turn(
        &mut self,
        turn_id: &str,
        input: &str,
        image_urls: &[String],
        association_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
    ) -> Result<Vec<ChatMessage>> {
        let base_projection = self.chat_base_context_projection(Some(turn_id))?;
        let projection = project_provider_turn_from_base_projection(
            base_projection,
            input,
            image_urls,
            association_prompt,
            auto_meme_reminder,
            0,
            self.context_char_budget,
        );
        self.last_dynamic_sources = projection.dynamic_sources.clone();
        self.state
            .enforce_provider_projection(Some(turn_id), &projection)?;
        Ok(projection.messages)
    }

    /// 构造 provider base context 投影。
    ///
    /// 参数:
    /// - `exclude_turn_id`: 需要从历史投影中排除的当前运行中轮次
    ///
    /// 返回:
    /// - provider base context 投影
    fn chat_base_context_projection(
        &self,
        exclude_turn_id: Option<&str>,
    ) -> Result<ProjectedBaseContext> {
        let loaded_tools_context = self.tool_visibility.loaded_context_prompt(&self.tools);
        let projected_history = self.state.project_history(exclude_turn_id)?;
        let compaction_summary_context = projected_history
            .checkpoint_context
            .or(self.state.compaction_summary_context()?);
        let last_auto_meme_reminder = memes::last_auto_meme_reminder(&self.config, &self.paths)?;
        let runtime_context = runtime_context_message();
        let epoch = self
            .state
            .context_epoch_projection(&self.base_system_prompt)?;
        Ok(project_provider_base_context_projection(
            &epoch.baseline,
            Some(self.mode.reminder()),
            selected_model_label(&self.config)?.as_deref(),
            loaded_tools_context.as_deref(),
            compaction_summary_context.as_deref(),
            projected_history.messages,
            last_auto_meme_reminder.as_deref(),
            &runtime_context,
        ))
    }
}
