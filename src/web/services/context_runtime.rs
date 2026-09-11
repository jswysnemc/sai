use crate::agent::{
    combine_context_updates, context_resource_update, context_resource_update_against_baseline,
    context_state_update, extract_instruction_files, load_instruction_prompt,
    plugin_context_updates, AgentMode, RuntimeContextSnapshot,
};
use crate::config::AppConfig;
use crate::llm::{ChatContent, ChatMessage};
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use anyhow::Result;

#[cfg(test)]
#[path = "context_runtime_tests.rs"]
mod tests;

/// Web 预览与用量统计共用的当前轮动态上下文。
pub(super) struct ContextRuntimeProjection {
    pub(super) goal_context: String,
    pub(super) compaction_summary: String,
    pub(super) runtime_context: String,
    pub(super) memory_index: String,
    pub(super) plugin_reply_context: String,
    pub(super) instruction_files: String,
    pub(super) memory_enabled: bool,
}

impl ContextRuntimeProjection {
    /// 判断当前投影是否包含动态上下文。
    ///
    /// 返回:
    /// - 至少存在一个非空动态段时返回 true
    pub(super) fn has_dynamic(&self) -> bool {
        [
            self.goal_context.as_str(),
            self.compaction_summary.as_str(),
            self.runtime_context.as_str(),
            self.memory_index.as_str(),
            self.plugin_reply_context.as_str(),
        ]
        .iter()
        .any(|part| !part.trim().is_empty())
    }
}

/// 【回复策略】【实时预览】独立只读实例读取当前上下文，不提供输入或模型服务
/// @param config 当前配置；paths 为应用目录；store 为会话；workspace_path 为目录；mode 为模式
/// @returns 可用于 Web 预览和用量估算的完整投影
pub(super) async fn project_context_runtime(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    workspace_path: &str,
    mode: AgentMode,
) -> Result<ContextRuntimeProjection> {
    let mut tools = crate::tools::readonly_registry(config, paths);
    tools.start_plugin_session(store.session_id())?;
    tools.inherit_plugin_storage_session(&store.state_dir().to_string_lossy());
    let prepared = crate::runtime_cwd::scope(
        std::path::PathBuf::from(workspace_path),
        tools.prepare_plugin_replies("", false),
    )
    .await;
    let contexts = tools.current_reply_contexts(&prepared.contexts);
    project_context_runtime_with_replies(config, paths, store, workspace_path, mode, &contexts)
}

/// 【回复策略】【同步用量】流式事件循环读取已载入正文，不在同步按键处理中等待异步策略
/// @param config 当前配置；paths 为应用目录；store 为会话；workspace_path 为目录；mode 为模式
/// @returns 当前已载入插件上下文的用量投影
pub(super) fn project_cached_context_runtime(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    workspace_path: &str,
    mode: AgentMode,
) -> Result<ContextRuntimeProjection> {
    let history = store.project_history(None)?;
    let checkpoint = history
        .checkpoint_context
        .or(store.compaction_summary_context()?);
    let mut contexts =
        crate::agent::plugin_context_snapshot(checkpoint.as_deref(), &history.messages);
    let ids = crate::tools::readonly_registry(config, paths).active_reply_policy_ids();
    contexts.retain(|id, _| ids.contains(id));
    project_context_runtime_with_replies(config, paths, store, workspace_path, mode, &contexts)
}

/// 按真实 Agent 请求路径投影当前轮动态上下文。
///
/// 参数:
/// - `config`: 已应用 Agent 与模型覆盖的运行配置
/// - `paths`: Sai 路径
/// - `store`: 当前会话状态
/// - `workspace_path`: 当前工作区路径
/// - `mode`: 当前运行模式
///
/// 返回:
/// - 不写入会话历史的动态上下文投影
fn project_context_runtime_with_replies(
    config: &AppConfig,
    paths: &SaiPaths,
    store: &StateStore,
    workspace_path: &str,
    mode: AgentMode,
    contexts: &std::collections::BTreeMap<String, String>,
) -> Result<ContextRuntimeProjection> {
    // 1. 读取 Goal、压缩摘要和最近一条用户输入
    let goal_context = if config.prompt_sections.state_contract {
        store
            .goal()?
            .map(|goal| crate::goal::system_context(&goal))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let projected_history = store.project_history(None)?;
    let compaction_summary = projected_history
        .checkpoint_context
        .clone()
        .or(store.compaction_summary_context()?)
        .unwrap_or_default();
    let latest_user_input = latest_user_text(&projected_history.messages);

    // 2. 只生成首次全量状态或相对历史发生变化的单项
    let selected_model = selected_model_label(config)?.unwrap_or_default();
    let snapshot = RuntimeContextSnapshot::capture(
        (!selected_model.trim().is_empty()).then_some(selected_model.as_str()),
        Some(config.tools.command_shell.as_str()),
    );
    let runtime_update = context_state_update(
        &snapshot,
        mode,
        (!compaction_summary.is_empty()).then_some(compaction_summary.as_str()),
        &projected_history.messages,
        config.prompt_sections.state_contract,
        config.prompt_sections.mode_reminder,
    )?;

    // 3. 复用真实请求的注入路径；索引全量注入，与当前输入无关
    let memory = config.memory_config();
    let memory_enabled =
        config.prompt_sections.memory_contract && memory.enabled && memory.association_enabled;
    let memory_index = if memory_enabled {
        MemoryStore::new(config, paths)
            .recall_for_turn(&latest_user_input, Some(workspace_path))?
            .unwrap_or_default()
    } else {
        String::new()
    };

    // 4. 【回复策略】【资源投影】调用方选择实时读取或使用已载入快照
    let plugin_reply_context = contexts.values().cloned().collect::<Vec<_>>().join("\n\n");
    let goal_update = if config.prompt_sections.state_contract {
        context_resource_update(
            "goal",
            &goal_context,
            (!compaction_summary.is_empty()).then_some(compaction_summary.as_str()),
            &projected_history.messages,
        )?
    } else {
        None
    };
    let plugin_update = plugin_context_updates(
        contexts,
        (!compaction_summary.is_empty()).then_some(compaction_summary.as_str()),
        &projected_history.messages,
    )?;
    let instruction_files = if config.load_instruction_files {
        load_instruction_prompt(paths)
    } else {
        String::new()
    };
    let baseline_files = store
        .context_epoch_baseline()?
        .as_deref()
        .map(extract_instruction_files)
        .unwrap_or_default();
    let instruction_update = if config.load_instruction_files {
        context_resource_update_against_baseline(
            "instruction_files",
            &instruction_files,
            &baseline_files,
            (!compaction_summary.is_empty()).then_some(compaction_summary.as_str()),
            &projected_history.messages,
        )?
    } else {
        None
    };
    let runtime_context = combine_context_updates([
        runtime_update,
        goal_update,
        instruction_update,
        plugin_update,
    ])
    .unwrap_or_default();

    Ok(ContextRuntimeProjection {
        goal_context,
        compaction_summary,
        runtime_context,
        memory_index,
        plugin_reply_context,
        instruction_files,
        memory_enabled,
    })
}

/// 构造当前配置的 provider/model 标签。
///
/// 参数:
/// - `config`: 已应用当前轮覆盖的配置
///
/// 返回:
/// - 当前 provider/model 标签
fn selected_model_label(config: &AppConfig) -> Result<Option<String>> {
    let provider = config.provider(None)?;
    let model = provider.default_model.trim();
    if model.is_empty() {
        return Ok(None);
    }
    let provider_name = provider.display_name.trim();
    let provider_label = if provider_name.is_empty() {
        provider.id.trim()
    } else {
        provider_name
    };
    if provider_label.is_empty() {
        Ok(Some(model.to_string()))
    } else {
        Ok(Some(format!("{provider_label}/{model}")))
    }
}

/// 提取历史中最近一条用户文本。
///
/// 参数:
/// - `messages`: 投影历史消息
///
/// 返回:
/// - 最近用户输入文本
fn latest_user_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| chat_content_text(message.content.as_ref()))
        .unwrap_or_default()
}

/// 提取消息中的纯文本内容。
///
/// 参数:
/// - `content`: 消息内容
///
/// 返回:
/// - 不含图片的文本
fn chat_content_text(content: Option<&ChatContent>) -> String {
    match content {
        Some(ChatContent::Text(text)) => text.clone(),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                crate::llm::ChatContentPart::Text { text } => Some(text.as_str()),
                crate::llm::ChatContentPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}
