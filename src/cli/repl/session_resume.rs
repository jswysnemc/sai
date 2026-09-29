use super::*;
use crate::state::ResumeTarget;

/// 【会话恢复】【运行环境】已准备完成的会话、Agent 和工具后台任务。
pub(super) struct ResumedSession {
    pub state: StateStore,
    pub agent: Agent,
    pub warmup: ReplToolWarmup,
}

/// 【会话恢复】【准备环境】在目标目录重建 Agent 和工具，失败时保留旧会话与目录。
/// 参数: paths/config/client/mode 为当前运行配置，target 为精确恢复目标；返回: 新运行环境
pub(super) fn prepare(
    paths: &SaiPaths,
    config: &AppConfig,
    client: &OpenAiCompatibleClient,
    mode: AgentMode,
    target: &ResumeTarget,
) -> Result<ResumedSession> {
    let directory = target.directory(paths)?;
    let change = crate::cli::session_resume::DirectoryChange::enter(paths, target)?;
    let state = StateStore::for_workspace_session(paths, &directory, &target.session.info.id)?;
    state.init_files()?;
    let registry = build_repl_tool_registry_without_mcp_for_session(
        config,
        paths,
        mode,
        state.session_id(),
        state.state_dir(),
    )?;
    let agent = Agent::new(
        config.clone(),
        paths,
        state.clone(),
        client.clone(),
        registry,
        mode,
    )?;
    crate::state::switch_workspace_session(paths, &directory, state.session_id())?;
    let warmup = ReplToolWarmup::start(
        config.clone(),
        paths.clone(),
        mode,
        state.session_id().into(),
        state.state_dir().into(),
    );
    change.commit();
    Ok(ResumedSession {
        state,
        agent,
        warmup,
    })
}
