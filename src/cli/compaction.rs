use super::{handle_agent_event, stream_render_options};
use crate::agent::AgentMode;
use crate::config::{AgentSurface, AppConfig};
use crate::paths::SaiPaths;
use crate::render;
use crate::runner::{ControlSubmission, RunnerSubmission, SubmissionSource};
use anyhow::Result;

/// 执行一次带终端动效和流式摘要的手动压缩。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 执行是否成功
pub(super) async fn run_compaction(paths: &SaiPaths) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = crate::config::apply_agent_override(
        AppConfig::load_or_default(paths)?,
        None,
        AgentSurface::Cli,
    )?;
    let reasoning_mode = render::ReasoningDisplayMode::from_config(&config.display.reasoning);
    let tool_call_mode = render::ToolCallDisplayMode::from_config(&config.display.tool_calls);
    let render_options = stream_render_options(&config);
    let mut renderer = render::StreamRenderer::new(
        reasoning_mode,
        tool_call_mode,
        false,
        render_options.clone(),
    );
    renderer.start_waiting()?;
    let submission = RunnerSubmission::control(
        SubmissionSource::Command,
        AgentMode::Yolo,
        ControlSubmission::new(crate::control_commands::ControlCommand::Compact),
    );
    let result = {
        let mut sink = |event: crate::runner::RunnerEvent| {
            if let crate::runner::RunnerEvent::Agent(agent_event) = event {
                handle_agent_event(&mut renderer, agent_event)?;
            }
            Ok(())
        };
        crate::runner::SessionRunner::new(paths)
            .with_config(config)
            .run_submission(submission, &mut sink)
            .await
    };
    renderer.finish()?;
    result.map(|_| ())
}
