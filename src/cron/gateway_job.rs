use super::CronJob;
use crate::agent::AgentMode;
use crate::cli::build_tool_registry;
use crate::config::AppConfig;
use crate::gateways::channel_context::load_session_channel_context;
use crate::gateways::channel_tools::{register_channel_message_tool, resolve_channel_target};
use crate::gateways::workspace::gateway_workspace_path;
use crate::paths::SaiPaths;
use crate::runner::{ChannelSubmission, RunnerSubmission, SessionRunner, SubmissionSource};
use anyhow::{Context, Result};

/// 在原渠道会话中执行定时任务并恢复渠道发送工具。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `job`: 到期定时任务
///
/// 返回:
/// - 执行是否成功
pub(crate) async fn run_gateway_job(paths: &SaiPaths, job: &CronJob) -> Result<()> {
    let workspace = gateway_workspace_path(paths);
    let context = load_session_channel_context(paths, &workspace, &job.session_id)?
        .with_context(|| format!("cron session has no channel context: {}", job.session_id))?;
    let config = crate::config::apply_agent_override(
        AppConfig::load_or_default(paths)?,
        None,
        crate::config::AgentSurface::Gateway,
    )?;
    let mut registry = build_tool_registry(&config, paths, AgentMode::Yolo)?;
    let current = resolve_channel_target(paths, &config, &context).await?;
    register_channel_message_tool(&mut registry, paths.clone(), config.clone(), current);
    let input = crate::runner::UserInputSubmission::new(job.prompt.clone(), AgentMode::Yolo)
        .with_extra_system_prompt(format!(
            "{}\n\n<scheduled-task>这是到期的渠道定时任务。完成任务后必须调用 send_channel_message，并使用 channel=current 把结果发送到原会话；不要只生成未发送的最终文本。</scheduled-task>",
            context.system_prompt()
        ));
    let channel = ChannelSubmission::new(context.channel())
        .with_inbound_marker(context.inbound_marker())
        .with_extra_loaded_tool("send_channel_message");
    let submission = RunnerSubmission::user_input(SubmissionSource::Gateway, input)
        .with_session_id(job.session_id.clone())
        .with_channel(channel);
    let mut sink = |_| Ok(());
    SessionRunner::new(paths)
        .with_config(config)
        .with_tool_registry(registry)
        .run_submission(submission, &mut sink)
        .await?;
    Ok(())
}
