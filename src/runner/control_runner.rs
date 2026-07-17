use super::{
    ActiveRunGuard, ControlSubmission, RunnerEvent, RunnerEventSink, RunnerSubmission, SessionOwner,
};
use crate::agent::{Agent, CompactionRunOutcome};
use crate::config::AppConfig;
use crate::llm::{ChatResult, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};

/// 使用独立 Agent 执行控制命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `config`: 本次运行配置
/// - `submission`: runner submission
/// - `control`: 控制命令
/// - `sink`: runner 事件接收器
///
/// 返回:
/// - 控制命令的空聊天结果
pub(super) async fn run_control<S>(
    paths: &SaiPaths,
    config: AppConfig,
    submission: &RunnerSubmission,
    control: ControlSubmission,
    sink: &mut S,
) -> Result<ChatResult>
where
    S: RunnerEventSink,
{
    AppConfig::init_files(paths)?;
    let context_limit_tokens = config.active_context_window_tokens()?;
    let state = match submission.session_id.as_deref() {
        Some(session_id) => StateStore::for_session(paths, session_id)?,
        None => StateStore::new(paths)?,
    };
    let state_dir = state.state_dir().to_path_buf();
    let active_run = ActiveRunGuard::acquire_with_state_dir(
        state.session_id(),
        SessionOwner::from(submission.source),
        &state_dir,
    )?;
    state.init_files()?;
    let client = OpenAiCompatibleClient::from_config(&config, paths)?;
    let agent = Agent::new(
        config,
        paths,
        state.clone(),
        client,
        ToolRegistry::new(),
        submission.mode,
    )?;
    sink.on_runner_event(RunnerEvent::Started)?;
    execute_control(&agent, control, sink).await?;
    let result = empty_result();
    sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
    if submission.show_final_summary {
        let mut snapshot = state.session_snapshot(context_limit_tokens)?;
        snapshot.active_run = Some(active_run.summary());
        sink.on_runner_event(RunnerEvent::FinalSummary(snapshot))?;
    }
    Ok(result)
}

/// 使用 REPL 已有 Agent 执行控制命令。
///
/// 参数:
/// - `submission`: runner submission
/// - `control`: 控制命令
/// - `agent`: 当前会话 Agent
/// - `sink`: runner 事件接收器
///
/// 返回:
/// - 控制命令的空聊天结果
pub(super) async fn run_control_with_agent<S>(
    submission: &RunnerSubmission,
    control: ControlSubmission,
    agent: &Agent,
    sink: &mut S,
) -> Result<ChatResult>
where
    S: RunnerEventSink,
{
    let state_dir = agent.state().state_dir().to_path_buf();
    let _active_run = ActiveRunGuard::acquire_with_state_dir(
        agent.session_id(),
        SessionOwner::from(submission.source),
        &state_dir,
    )?;
    sink.on_runner_event(RunnerEvent::Started)?;
    execute_control(agent, control, sink).await?;
    let result = empty_result();
    sink.on_runner_event(RunnerEvent::Completed(result.clone()))?;
    Ok(result)
}

/// 执行已识别的控制命令。
///
/// 参数:
/// - `agent`: 当前 Agent
/// - `control`: 控制命令
/// - `sink`: runner 事件接收器
///
/// 返回:
/// - 压缩执行结果
async fn execute_control<S>(
    agent: &Agent,
    control: ControlSubmission,
    sink: &mut S,
) -> Result<CompactionRunOutcome>
where
    S: RunnerEventSink,
{
    match control.command {
        crate::control_commands::ControlCommand::Compact => agent
            .compact_conversation_now(&mut |event| {
                sink.on_runner_event(RunnerEvent::Agent(event))
            })
            .await,
        command => bail!("runner control submission is not supported: {command:?}"),
    }
}

/// 构造不参与助手消息持久化的控制命令结果。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 空聊天结果
fn empty_result() -> ChatResult {
    ChatResult {
        content: String::new(),
        reasoning: None,
        usage: None,
        tool_calls: Vec::new(),
    }
}
