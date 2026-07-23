use crate::agent::{Agent, AgentMode};
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::runner::{ChannelSubmission, UserInputSubmission};
use crate::state::StateStore;
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::collections::BTreeSet;

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
pub(super) fn build_agent(
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
pub(super) fn loaded_tools_for_submission(
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
pub(super) fn merge_loaded_tools(
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
pub(super) fn with_channel_marker(
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

/// 会话仍为默认/启发式标题时，调用小模型生成标题。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `state`: 会话状态
/// - `input`: 用户输入
/// - `assistant_preview`: 助手回复预览
///
/// 返回:
/// - 无
pub(super) async fn try_auto_title_session(
    paths: &SaiPaths,
    config: &AppConfig,
    state: &StateStore,
    input: &UserInputSubmission,
    assistant_preview: &str,
) {
    if input.automatic_input.is_some() {
        return;
    }
    let user_message = input.input.trim();
    if user_message.is_empty() {
        return;
    }
    let session_id = state.session_id().to_string();
    let Ok(sessions) = crate::state::list_sessions(paths) else {
        return;
    };
    let Some(session) = sessions.iter().find(|item| item.id == session_id) else {
        return;
    };
    let _ = crate::assistants::maybe_auto_title_session(
        paths,
        config,
        &session_id,
        &session.title,
        user_message,
        Some(assistant_preview),
    )
    .await;
}
