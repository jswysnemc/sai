use super::{
    ActiveRunGuard, ChannelSubmission, RunnerEvent, RunnerEventSink, RunnerSubmission,
    RunnerSubmissionKind, SessionOwner, SubmissionSource, TurnRunner, UserInputSubmission,
};
use super::submission_tools::{
    build_submission_tool_registry, should_apply_command_mode_exit_policy, should_discover_mcp,
};
use crate::agent::{Agent, AgentMode};
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::perf_trace::PerfTrace;
use crate::permission::{PermissionAuditLog, PermissionProfile};
use crate::state::StateStore;
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use std::collections::BTreeSet;

/// 会话 runner，负责会话范围资源和单轮执行边界。
pub(crate) struct SessionRunner<'paths> {
    paths: &'paths SaiPaths,
    config_override: Option<AppConfig>,
    tool_registry_override: Option<ToolRegistry>,
}

impl<'paths> SessionRunner<'paths> {
    /// 创建会话 runner。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 会话 runner
    pub(crate) fn new(paths: &'paths SaiPaths) -> Self {
        Self {
            paths,
            config_override: None,
            tool_registry_override: None,
        }
    }

    /// 设置本次运行使用的配置覆盖。
    ///
    /// 参数:
    /// - `config`: 已由入口处理过临时覆盖的应用配置
    ///
    /// 返回:
    /// - 更新后的会话 runner
    pub(crate) fn with_config(mut self, config: AppConfig) -> Self {
        self.config_override = Some(config);
        self
    }

    /// 设置本次运行使用的工具注册表覆盖。
    ///
    /// 参数:
    /// - `registry`: 已由入口补充过渠道工具的工具注册表
    ///
    /// 返回:
    /// - 更新后的会话 runner
    pub(crate) fn with_tool_registry(mut self, registry: ToolRegistry) -> Self {
        self.tool_registry_override = Some(registry);
        self
    }

    /// 执行 runner submission。
    ///
    /// 参数:
    /// - `submission`: runner 输入
    /// - `sink`: runner 事件接收器
    ///
    /// 返回:
    /// - 可选聊天结果，控制命令后续接入
    pub(crate) async fn run_submission<S>(
        &self,
        submission: RunnerSubmission,
        sink: &mut S,
    ) -> Result<Option<crate::llm::ChatResult>>
    where
        S: RunnerEventSink,
    {
        match &submission.kind {
            RunnerSubmissionKind::UserInput(input) => self
                .run_user_input(&submission, input.clone(), sink)
                .await
                .map(Some),
            RunnerSubmissionKind::Control(control) => super::control_runner::run_control(
                self.paths,
                self.load_config()?,
                &submission,
                control.clone(),
                sink,
            )
            .await
            .map(Some),
        }
    }

    /// 在复用的 Agent 上执行 submission（REPL 热路径，避免每轮重建）。
    ///
    /// 参数:
    /// - `submission`: runner 输入
    /// - `agent`: 长生命周期 Agent
    /// - `sink`: runner 事件接收器
    ///
    /// 返回:
    /// - 可选聊天结果
    pub(crate) async fn run_submission_with_agent<S>(
        &self,
        submission: RunnerSubmission,
        agent: &mut Agent,
        sink: &mut S,
    ) -> Result<Option<crate::llm::ChatResult>>
    where
        S: RunnerEventSink,
    {
        match &submission.kind {
            RunnerSubmissionKind::UserInput(input) => self
                .run_user_input_with_agent(&submission, input.clone(), agent, sink)
                .await
                .map(Some),
            RunnerSubmissionKind::Control(control) => {
                super::control_runner::run_control_with_agent(
                    &submission,
                    control.clone(),
                    agent,
                    sink,
                )
                .await
                .map(Some)
            }
        }
    }

    /// 执行用户输入 submission。
    ///
    /// 参数:
    /// - `submission`: runner 输入
    /// - `input`: 用户输入 submission
    /// - `sink`: runner 事件接收器
    ///
    /// 返回:
    /// - 聊天结果
    async fn run_user_input<S>(
        &self,
        submission: &RunnerSubmission,
        input: UserInputSubmission,
        sink: &mut S,
    ) -> Result<crate::llm::ChatResult>
    where
        S: RunnerEventSink,
    {
        AppConfig::init_files(self.paths)?;
        let mut perf = PerfTrace::new("runner");
        perf.mark("start submission");
        let config = self.load_config()?;
        let context_limit_chars = config.active_context_window_tokens()?;
        let state = match submission.session_id.as_deref() {
            Some(session_id) => StateStore::for_session(self.paths, session_id)?,
            None => StateStore::new(self.paths)?,
        };
        let state_dir = state.state_dir().to_path_buf();
        let _active_run = ActiveRunGuard::acquire_with_state_dir(
            state.session_id(),
            SessionOwner::from(submission.source),
            &state_dir,
        )?;
        state.init_files()?;
        let client = OpenAiCompatibleClient::from_config(&config, self.paths)?;
        let registry = self.load_tool_registry(
            &config,
            submission.source,
            input.mode,
            state.session_id(),
            state.state_dir(),
        )?;
        let input = with_channel_marker(input, submission.channel.as_ref());
        let mut agent = build_agent(
            config.clone(),
            self.paths,
            state.clone(),
            client,
            registry,
            input.mode,
            input.extra_system_prompt.as_deref(),
        )?;
        if config.tools.progressive_loading_enabled {
            let loaded_tools = loaded_tools_for_submission(&state, submission.channel.as_ref())?;
            agent.restore_loaded_tools(&loaded_tools);
            sink.on_runner_event(RunnerEvent::LoadedToolsChanged(loaded_tools))?;
        }
        sink.on_runner_event(RunnerEvent::Started)?;
        let mut turn_runner = TurnRunner::for_source(&mut agent, submission.source);
        let result = turn_runner.run_user_input(&input, sink).await;
        perf.mark("turn runner done");
        if config.tools.progressive_loading_enabled {
            let loaded_tools = agent.loaded_tools();
            state.save_loaded_tools(&loaded_tools)?;
            sink.on_runner_event(RunnerEvent::LoadedToolsChanged(loaded_tools))?;
        }
        let result = result?;
        if should_apply_command_mode_exit_policy(submission.source) {
            state.apply_command_mode_runtime_exit_policy()?;
            perf.mark("runtime exit policy");
        }
        if submission.show_final_summary {
            let mut snapshot = state.session_snapshot(context_limit_chars)?;
            perf.mark("session snapshot");
            snapshot.dynamic_sources = agent.last_dynamic_sources();
            snapshot.active_run = Some(_active_run.summary());
            sink.on_runner_event(RunnerEvent::FinalSummary(snapshot))?;
            perf.mark("final summary event");
        }
        Ok(result)
    }

    /// 在既有 Agent 上执行用户输入（不重建 MemoryStore / 工具注册表 / 客户端）。
    ///
    /// 参数:
    /// - `submission`: runner 输入
    /// - `input`: 用户输入 submission
    /// - `agent`: 复用中的 Agent
    /// - `sink`: runner 事件接收器
    ///
    /// 返回:
    /// - 聊天结果
    async fn run_user_input_with_agent<S>(
        &self,
        submission: &RunnerSubmission,
        input: UserInputSubmission,
        agent: &mut Agent,
        sink: &mut S,
    ) -> Result<crate::llm::ChatResult>
    where
        S: RunnerEventSink,
    {
        AppConfig::init_files(self.paths)?;
        let mut perf = PerfTrace::new("runner");
        perf.mark("start reused-agent submission");
        let config = self.load_config()?;
        let context_limit_chars = config.active_context_window_tokens()?;
        let state_dir = agent.state().state_dir().to_path_buf();
        // 1. 仍按轮获取运行所有权，避免并发会话互相踩
        let _active_run = ActiveRunGuard::acquire_with_state_dir(
            agent.session_id(),
            SessionOwner::from(submission.source),
            &state_dir,
        )?;
        sink.on_runner_event(RunnerEvent::Started)?;
        let input = with_channel_marker(input, submission.channel.as_ref());
        // 2. 执行单轮（Agent 已由 REPL 完成 prepare_for_turn / switch_mode）
        let mut turn_runner = TurnRunner::for_source(agent, submission.source);
        let result = turn_runner.run_user_input(&input, sink).await;
        perf.mark("turn runner done");
        if config.tools.progressive_loading_enabled {
            let loaded_tools = agent.loaded_tools();
            // 3. 持久化渐进加载集合，供崩溃恢复
            agent.state().save_loaded_tools(&loaded_tools)?;
            sink.on_runner_event(RunnerEvent::LoadedToolsChanged(loaded_tools))?;
        }
        let result = result?;
        if should_apply_command_mode_exit_policy(submission.source) {
            agent.state().apply_command_mode_runtime_exit_policy()?;
            perf.mark("runtime exit policy");
        }
        if submission.show_final_summary {
            let mut snapshot = agent.state().session_snapshot(context_limit_chars)?;
            perf.mark("session snapshot");
            snapshot.dynamic_sources = agent.last_dynamic_sources();
            snapshot.active_run = Some(_active_run.summary());
            sink.on_runner_event(RunnerEvent::FinalSummary(snapshot))?;
            perf.mark("final summary event");
        }
        Ok(result)
    }

    /// 读取本次 runner 使用的配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 应用配置
    fn load_config(&self) -> Result<AppConfig> {
        match &self.config_override {
            Some(config) => Ok(config.clone()),
            None => AppConfig::load_or_default(self.paths),
        }
    }

    /// 读取本次 runner 使用的工具注册表。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `source`: submission 来源
    /// - `mode`: Agent 模式
    /// - `session_id`: 会话 ID
    /// - `state_dir`: 会话状态目录
    ///
    /// 返回:
    /// - 工具注册表
    fn load_tool_registry(
        &self,
        config: &AppConfig,
        source: SubmissionSource,
        mode: AgentMode,
        session_id: &str,
        state_dir: &std::path::Path,
    ) -> Result<ToolRegistry> {
        let mut registry = match &self.tool_registry_override {
            Some(registry) => Ok(registry.clone()),
            None => build_submission_tool_registry(
                config, self.paths, source, mode, session_id, state_dir,
            ),
        }?;
        if mode != AgentMode::Plan && source == SubmissionSource::Gateway {
            crate::cron::register_tool(&mut registry, self.paths.clone(), session_id.to_string());
        }
        let mut selected = if let Some(runtime) = config.agent_runtime.as_ref() {
            let allowed = runtime
                .enabled_tools
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut filtered = registry.clone_filtered(&allowed);
            let required = match source {
                SubmissionSource::Repl | SubmissionSource::Web => {
                    &["subagent", "todo", "ask_question"][..]
                }
                SubmissionSource::Gateway => &["cron", "send_channel_message"][..],
                SubmissionSource::Command | SubmissionSource::ShellIntercept => {
                    &["ask_question"][..]
                }
            };
            for name in required {
                if registry.contains(name) {
                    filtered.register_from(&registry, name)?;
                }
            }
            filtered
        } else {
            registry
        };
        let workspace = crate::runtime_cwd::current_dir()?;
        let profile_mode = mode.permission_profile_mode();
        let audit = (mode != AgentMode::Yolo).then(|| {
            PermissionAuditLog::new(
                state_dir.join("permission-audit.jsonl"),
                session_id.to_string(),
            )
        });
        selected.set_permission_profile(PermissionProfile::new(profile_mode, workspace, audit));
        Ok(selected)
    }
}

/// 构造 Agent。
///

/// 构造 Agent。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
/// - `state`: 状态存储
/// - `client`: LLM 客户端
/// - `registry`: 工具注册表
/// - `mode`: Agent 模式
/// - `extra_system_prompt`: 额外系统提示词
///
/// 返回:
/// - Agent 实例
fn build_agent(
    config: AppConfig,
    paths: &SaiPaths,
    state: StateStore,
    client: OpenAiCompatibleClient,
    registry: ToolRegistry,
    mode: AgentMode,
    extra_system_prompt: Option<&str>,
) -> Result<Agent> {
    if extra_system_prompt.is_some() {
        Agent::new_with_extra_system_prompt(
            config,
            paths,
            state,
            client,
            registry,
            mode,
            extra_system_prompt,
        )
    } else {
        Agent::new(config, paths, state, client, registry, mode)
    }
}

/// 读取当前 submission 的已加载工具集合。
///
/// 参数:
/// - `state`: 状态存储
/// - `channel`: 可选渠道元数据
///
/// 返回:
/// - 已加载工具集合
fn loaded_tools_for_submission(
    state: &StateStore,
    channel: Option<&ChannelSubmission>,
) -> Result<Vec<String>> {
    let loaded_tools = state.load_loaded_tools()?;
    Ok(merge_loaded_tools(loaded_tools, channel))
}

/// 合并状态内和渠道要求的已加载工具。
///
/// 参数:
/// - `loaded_tools`: 状态内已加载工具
/// - `channel`: 可选渠道元数据
///
/// 返回:
/// - 去重后的已加载工具
fn merge_loaded_tools(
    loaded_tools: Vec<String>,
    channel: Option<&ChannelSubmission>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();
    for tool in loaded_tools.into_iter().chain(
        channel
            .into_iter()
            .flat_map(|channel| channel.extra_loaded_tools.iter().cloned()),
    ) {
        if seen.insert(tool.clone()) {
            merged.push(tool);
        }
    }
    merged
}

/// 将渠道入站标记加入用户输入。
///
/// 参数:
/// - `input`: 用户输入 submission
/// - `channel`: 可选渠道元数据
///
/// 返回:
/// - 更新后的用户输入 submission
fn with_channel_marker(
    mut input: UserInputSubmission,
    channel: Option<&ChannelSubmission>,
) -> UserInputSubmission {
    if let Some(marker) = channel.and_then(|channel| channel.inbound_marker.as_deref()) {
        input.extra_system_prompt = Some(match input.extra_system_prompt.take() {
            Some(prompt) => format!("{prompt}\n\n{marker}"),
            None => marker.to_string(),
        });
    }
    input
}

#[cfg(test)]
#[path = "session_runner_gateway_tests.rs"]
mod gateway_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

    /// 创建测试路径集合。
    ///
    /// 参数:
    /// - `state_dir`: 状态目录
    ///
    /// 返回:
    /// - 测试路径集合
    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    /// 验证渠道入站标记会被前置到用户输入。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn channel_marker_is_prepended_to_user_input() {
        let input = UserInputSubmission::new("你好", AgentMode::Yolo);
        let channel = ChannelSubmission::new("qq")
            .with_inbound_marker("[channel=qq gateway=qq-bot target=group]");

        let input = with_channel_marker(input, Some(&channel));

        assert_eq!(input.input, "你好");
        assert_eq!(
            input.extra_system_prompt.as_deref(),
            Some("[channel=qq gateway=qq-bot target=group]")
        );
    }

    /// 验证渠道工具和已有工具会按首次出现顺序去重。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn channel_loaded_tools_are_merged_without_duplicates() {
        let loaded_tools = vec!["read_file".to_string(), "send_channel_message".to_string()];
        let channel = ChannelSubmission::new("qq")
            .with_extra_loaded_tool("send_channel_message")
            .with_extra_loaded_tool("write_file");

        let merged = merge_loaded_tools(loaded_tools, Some(&channel));

        assert_eq!(
            merged,
            vec!["read_file", "send_channel_message", "write_file"]
        );
    }

    /// 验证子智能体工具只在交互式 REPL 和 Web 来源中启用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn subagent_tool_is_limited_to_interactive_sources() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let config = AppConfig::default();
        let interactive_sources = [SubmissionSource::Repl, SubmissionSource::Web];
        let non_interactive_sources = [
            SubmissionSource::Command,
            SubmissionSource::Gateway,
            SubmissionSource::ShellIntercept,
        ];

        // 1. 交互式来源必须提供子智能体工具
        for source in interactive_sources {
            let registry = build_submission_tool_registry(
                &config,
                &paths,
                source,
                AgentMode::Yolo,
                "interactive-session",
                std::path::Path::new("."),
            )
            .unwrap();
            assert!(registry.contains("subagent"), "source: {source:?}");
            assert!(registry.contains("todo"), "source: {source:?}");
        }

        // 2. 非交互式来源不得提供子智能体工具
        for source in non_interactive_sources {
            let registry = build_submission_tool_registry(
                &config,
                &paths,
                source,
                AgentMode::Yolo,
                "non-interactive-session",
                std::path::Path::new("."),
            )
            .unwrap();
            assert!(!registry.contains("subagent"), "source: {source:?}");
            assert!(!registry.contains("todo"), "source: {source:?}");
        }
    }

    /// 验证短生命周期命令入口不会在模型请求前同步发现 MCP。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn short_lived_command_sources_skip_eager_mcp_discovery() {
        assert!(!should_discover_mcp(SubmissionSource::Command));
        assert!(!should_discover_mcp(SubmissionSource::ShellIntercept));
        assert!(should_discover_mcp(SubmissionSource::Repl));
        assert!(should_discover_mcp(SubmissionSource::Web));
        assert!(should_discover_mcp(SubmissionSource::Gateway));
    }

    /// 验证定时任务管理工具只对 Gateway 来源开放。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn cron_tool_is_limited_to_gateway_source() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let config = AppConfig::default();
        let runner = SessionRunner::new(&paths);
        let sources = [
            SubmissionSource::Command,
            SubmissionSource::Repl,
            SubmissionSource::Web,
            SubmissionSource::Gateway,
            SubmissionSource::ShellIntercept,
        ];

        for source in sources {
            let registry = runner
                .load_tool_registry(
                    &config,
                    source,
                    AgentMode::Yolo,
                    "cron-session",
                    std::path::Path::new("."),
                )
                .unwrap();
            assert_eq!(
                registry.contains("cron"),
                source == SubmissionSource::Gateway,
                "source: {source:?}"
            );
        }
    }

    #[test]
    fn surface_tools_survive_agent_runtime_whitelist() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let mut config = AppConfig::default();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: vec!["read_file".to_string()],
            skills_full: Vec::new(),
            skills_named: Vec::new(),
        });
        let runner = SessionRunner::new(&paths);

        for source in [SubmissionSource::Repl, SubmissionSource::Web] {
            let registry = runner
                .load_tool_registry(
                    &config,
                    source,
                    AgentMode::Yolo,
                    "interactive",
                    std::path::Path::new("."),
                )
                .unwrap();
            assert!(registry.contains("subagent"));
            assert!(registry.contains("todo"));
        }
        let gateway = runner
            .load_tool_registry(
                &config,
                SubmissionSource::Gateway,
                AgentMode::Yolo,
                "gateway",
                std::path::Path::new("."),
            )
            .unwrap();
        assert!(gateway.contains("cron"));
    }

    /// 验证命令模式后台命令会绑定 command-mode owner。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn command_mode_registry_marks_background_owner() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let config = AppConfig::default();
        let state = StateStore::new(&paths).unwrap();
        let registry = build_submission_tool_registry(
            &config,
            &paths,
            SubmissionSource::Command,
            AgentMode::Yolo,
            state.session_id(),
            state.state_dir(),
        )
        .unwrap();

        registry
            .call(
                "background_command",
                r#"{"action":"start","command":"true","label":"cmd-owner"}"#,
            )
            .await
            .unwrap();

        let db_path = crate::state::active_state_dir(&paths)
            .unwrap()
            .join("conversation.db");
        let conn = rusqlite::Connection::open(db_path).unwrap();
        let owner_kind: String = conn
            .query_row(
                "SELECT owner_kind
                 FROM runtime_processes
                 ORDER BY started_at DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(owner_kind, "command_mode");
    }
}
