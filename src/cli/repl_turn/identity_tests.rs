use super::*;
use crate::agent::AgentMode;
use crate::runner::{SubmissionSource, UserInputSubmission};
use crate::web::runs::EventAssembler;

#[path = "../../runner/automatic_test_support.rs"]
#[allow(dead_code)]
mod support;

/// 【会话同步】【终端验证】通过真实 Runner 和本地模型接口验证事件与落盘标识一致。
/// 参数: 无
/// 返回: 无；同一条消息出现不同标识时断言失败
#[tokio::test]
async fn terminal_events_match_persisted_turn_identity() {
    let mut harness = support::AutomaticTestHarness::new().await;
    let mut submission = RunnerSubmission::user_input(
        SubmissionSource::Repl,
        UserInputSubmission::new("检查会话同步", AgentMode::Yolo),
    );
    let (run_id, text, images) = prepare_run_identity(&mut submission);
    let RunnerSubmissionKind::UserInput(input) = submission.kind else {
        unreachable!()
    };
    harness.submit(input).await.unwrap();
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run(&run_id, &text, &images);
    let events: Vec<_> = harness
        .events
        .iter()
        .cloned()
        .flat_map(|event| assembler.map(event))
        .collect();
    let turns = harness.agent.state().session_timeline(10).unwrap();
    let persisted_id = &turns.last().unwrap().turn_id;
    assert_eq!(&run_id, persisted_id);
    assert!(events.iter().all(|event| &event.run_id == persisted_id));
}

/// 【会话同步】【远端验证】终端接手 Web 提交后继续使用请求入口分配的标识。
/// 参数: 无
/// 返回: 无；标识被重新生成时断言失败
#[test]
fn forwarded_turn_keeps_original_identity() {
    let mut submission = RunnerSubmission::user_input(
        SubmissionSource::Repl,
        UserInputSubmission::new("重发消息", AgentMode::Yolo).with_turn_id("web-run"),
    );
    assert_eq!(prepare_run_identity(&mut submission).0, "web-run");
}
