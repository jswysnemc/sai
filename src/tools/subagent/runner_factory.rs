use super::*;

/// 【子任务】【模型刷新】启动前读取最新档案及子任务设置，保留主对话当前模型和思考覆盖。
///
/// 参数: `context` 为即将启动的子任务上下文
/// 返回: 配置读取结果；未保存配置时沿用上下文
pub(super) fn refresh_model_settings(context: &mut SubagentContext) -> Result<()> {
    if !context.paths.config_file.exists() {
        return Ok(());
    }
    let saved = AppConfig::load(&context.paths)?;
    let main = context.config.provider(None)?.clone();
    // 1. 【子任务】【模型刷新】档案是类型专用设置缺省时的模型来源，必须一并刷新
    context.config.agents = saved.agents;
    context.config.subagent = saved.subagent;
    context.config.providers = saved.providers;
    if let Some(provider) = context
        .config
        .providers
        .iter_mut()
        .find(|item| item.id == main.id)
    {
        provider.default_model = main.default_model;
        provider.thinking_level = main.thinking_level;
    } else {
        context.config.providers.push(main);
    }
    Ok(())
}

/// 构造子代理执行器（模型客户端、工具集、系统提示与消息注入回调）。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID，用于步间轮询其消息队列
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 每个任务段的最大工具调用次数
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
/// - `persistent`: 是否为持久会话（持久会话不设整体运行时限提醒）
///
/// 返回:
/// - 可直接运行的子代理执行器
pub(super) fn build_subagent_runner(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    context: &SubagentContext,
    tool_progress: ToolProgress,
    persistent: bool,
) -> Result<SubagentRunner> {
    // 1. 按子智能体模型配置构造客户端,未配置时沿用主对话供应商与模型
    let profile = context
        .config
        .resolve_registered_agent(Some(subagent_type))
        .with_context(|| format!("subagent profile is not exposed: {subagent_type}"))?;
    let client = build_subagent_client(context, &profile)?;
    let (default_prompt, default_tools, excluded) = match subagent_type {
        "explore" => (
            EXPLORE_PROMPT,
            context.tools.clone_filtered(EXPLORE_ALLOWED),
            Vec::new(),
        ),
        "general" => (
            GENERAL_PROMPT,
            context.tools.clone(),
            GENERAL_EXCLUDED.to_vec(),
        ),
        _ => (
            GENERAL_PROMPT,
            context.tools.clone(),
            GENERAL_EXCLUDED.to_vec(),
        ),
    };
    let tools = if inherits_default_tools(&context.config, &profile) {
        default_tools
    } else {
        let allowed = profile
            .enabled_tools
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        default_tools.clone_filtered(&allowed)
    };
    // 2. 渐进网关按子 Agent 的实际工具集合和延迟配置重建，避免沿用主 Agent 的描述
    let mut tools = tools.clone_excluding(&[crate::tools::LOAD_NAME, crate::tools::INVOKE_NAME]);
    crate::tools::progressive::register_loader(&mut tools, &profile.deferred_tools);
    let base_prompt = if profile.system_prompt.trim().is_empty() {
        default_prompt.to_string()
    } else {
        profile.system_prompt.clone()
    };
    let system_prompt = subagent_system_prompt(context, &profile, &base_prompt)?;
    // 3. 以 Full 模式上报,时间线可拿到工具调用参数、结果与流式文本
    let progress = SubagentProgress::new(tool_progress, ProgressMode::Full, true);
    // 4. 步间消息注入:主代理 action=send 与用户留言经消息队列在此进入对话
    let poll_id = subagent_id.to_string();
    let message_poll: crate::tools::subagent_runner::SubagentMessagePoll =
        std::sync::Arc::new(move || {
            subagent_state::drain_subagent_inbox(&poll_id)
                .into_iter()
                .map(
                    |message| crate::tools::subagent_runner::SubagentInjectedMessage {
                        from: message.from,
                        text: message.text,
                    },
                )
                .collect()
        });
    Ok(SubagentRunner::new(client, &system_prompt, tools, progress)
        .progressive_loading(
            profile.deferred_tools.clone(),
            context.config.clone(),
            context.paths.clone(),
        )
        .max_steps(max_steps)
        .timeout_seconds(TOOL_TIMEOUT_SECONDS)
        // 持久会话面向多任务段长期存活,不注入整体时限收尾提醒
        .session_deadline_seconds(if persistent {
            0
        } else {
            SUBAGENT_TIMEOUT_SECONDS
        })
        .excluded_tools(&excluded)
        .with_message_poll(message_poll))
}

/// 判断子 Agent 是否应沿用类型内置的工具集合。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `profile`: 已解析的统一 Agent 档案
///
/// 返回:
/// - 内置 Agent 或旧版迁移档案在工具为空时返回 true
pub(super) fn inherits_default_tools(
    config: &AppConfig,
    profile: &crate::config::AgentProfile,
) -> bool {
    profile.enabled_tools.is_empty()
        && (matches!(profile.id.as_str(), "general" | "explore")
            || !config.agents.iter().any(|agent| agent.id == profile.id))
}

/// 组合 Agent 系统提示词与该档案启用的 Skills。
///
/// 参数:
/// - `context`: 子智能体运行上下文
/// - `profile`: 统一 Agent 档案
/// - `base_prompt`: Agent 基础系统提示词
///
/// 返回:
/// - 可直接交给子智能体的完整系统提示词
fn subagent_system_prompt(
    context: &SubagentContext,
    profile: &crate::config::AgentProfile,
    base_prompt: &str,
) -> Result<String> {
    if profile.skills_full.is_empty() && profile.skills_named.is_empty() {
        return Ok(base_prompt.to_string());
    }
    let mut config = context.config.clone();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: profile.enabled_tools.clone(),
        exclusive: profile.tools_exclusive,
        deferred_tools: profile.deferred_tools.clone(),
        skills_full: profile.skills_full.clone(),
        skills_named: profile.skills_named.clone(),
    });
    let skills = crate::tools::skills_prompt(&config, &context.paths)?;
    if skills.trim().is_empty() {
        Ok(base_prompt.to_string())
    } else {
        Ok(format!("{base_prompt}\n\n{skills}"))
    }
}
