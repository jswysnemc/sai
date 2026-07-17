#![allow(dead_code, unused_imports)]

mod continuation;
mod control_runner;
mod events;
mod ownership;
mod session_runner;
mod submission;
mod turn_runner;

use crate::paths::SaiPaths;
use anyhow::Result;

pub(crate) use continuation::{ContinuationReason, RunnerContinuation};
pub(crate) use events::{RunnerEvent, RunnerEventSink, RunnerOutput};
pub(crate) use ownership::{ActiveRunGuard, SessionOwner};
pub(crate) use session_runner::SessionRunner;
pub(crate) use submission::{
    ChannelSubmission, ControlSubmission, RenderPolicy, RunnerSubmission, RunnerSubmissionKind,
    SubmissionSource, UserInputSubmission,
};
pub(crate) use turn_runner::TurnRunner;

/// 执行一条 runner submission。
///
/// 参数:
/// - `submission`: 已归一化的 runner 输入
///
/// 返回:
/// - runner 输出事件和完成结果
pub(crate) async fn run_submission(
    paths: &SaiPaths,
    submission: RunnerSubmission,
) -> Result<RunnerOutput> {
    let mut output = RunnerOutput::default();
    {
        let mut collector = |event| {
            output.push_event(event);
            Ok(())
        };
        SessionRunner::new(paths)
            .run_submission(submission, &mut collector)
            .await?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::control_commands::ControlCommand;

    /// 验证命令模式输入可以构造成统一 submission。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn builds_command_submission() {
        let submission = RunnerSubmission::user_input(
            SubmissionSource::Command,
            UserInputSubmission::new("整理当前项目", AgentMode::Yolo),
        )
        .with_final_summary(true);

        assert_eq!(submission.source, SubmissionSource::Command);
        assert_eq!(submission.mode, AgentMode::Yolo);
        assert!(submission.show_final_summary);
    }

    /// 验证 REPL 输入可以构造成统一 submission。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn builds_repl_submission() {
        let submission = RunnerSubmission::user_input(
            SubmissionSource::Repl,
            UserInputSubmission::new("继续", AgentMode::Plan),
        );

        assert_eq!(submission.source, SubmissionSource::Repl);
        assert_eq!(submission.mode, AgentMode::Plan);
        assert!(!submission.show_final_summary);
    }

    /// 验证 gateway 输入可以携带渠道元数据。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn builds_gateway_submission_with_channel_metadata() {
        let channel = ChannelSubmission::new("qq")
            .with_inbound_marker("[channel=qq gateway=qq-bot target=group]")
            .with_extra_loaded_tool("send_channel_message");
        let submission = RunnerSubmission::user_input(
            SubmissionSource::Gateway,
            UserInputSubmission::new("用户消息", AgentMode::Yolo),
        )
        .with_channel(channel);

        assert_eq!(submission.source, SubmissionSource::Gateway);
        assert_eq!(
            submission
                .channel
                .as_ref()
                .and_then(|channel| { channel.extra_loaded_tools.first().map(String::as_str) }),
            Some("send_channel_message")
        );
    }

    /// 验证控制命令可以进入 runner submission 边界。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn builds_control_submission() {
        let submission = RunnerSubmission::control(
            SubmissionSource::Repl,
            AgentMode::Yolo,
            ControlSubmission::new(ControlCommand::Compact),
        );

        assert!(matches!(
            submission.kind,
            RunnerSubmissionKind::Control(ControlSubmission {
                command: ControlCommand::Compact
            })
        ));
    }

    /// 验证 runner 输出会记录 completion，gateway 可读取正文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn runner_output_records_completion_text() {
        let mut output = RunnerOutput::default();
        output.push_event(RunnerEvent::Completed(crate::llm::ChatResult {
            content: "回复正文".to_string(),
            reasoning: None,
            usage: None,
            tool_calls: Vec::new(),
        }));

        assert_eq!(
            output
                .completion
                .as_ref()
                .map(|result| result.content.as_str()),
            Some("回复正文")
        );
    }
}
