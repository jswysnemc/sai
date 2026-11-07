use super::*;
use crate::control_commands::{
    execute_goal_command, parse_control_command, ControlCommand, ControlSurface,
};

/// 【目标】【提交测试】按实际命令路径执行提交并检查持久化正文与连续两次续轮。
/// 参数: `submission` 为输入提交，`expected` 为完整目标；返回: 无，正文丢失时断言失败。
fn assert_goal_round_trip(submission: &ReplInputSubmission, expected: &str) {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(temp.path());
    let state = crate::state::StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let Some(ControlCommand::Goal(command)) =
        parse_control_command(submission.command_input(), ControlSurface::Repl).unwrap()
    else {
        panic!("提交没有解析为目标命令");
    };
    assert!(
        execute_goal_command(&state, command)
            .unwrap()
            .should_continue
    );
    for _ in 0..2 {
        let goal = state.goal().unwrap().unwrap();
        assert_eq!(goal.objective, expected);
        assert_eq!(goal.token_budget, Some(5000));
        let prompt = crate::goal::continuation_prompt(&goal);
        assert!(prompt.contains(expected));
        assert!(!prompt.contains("[text 1 "));
        state.append_goal_progress("继续检查需求").unwrap();
    }
}

/// 【目标】【提交测试】先输入命令再粘贴长文本时，正文原位展开且保留历史附件。
/// 参数: 无；返回: 无，目标、预算或输入快照丢失时断言失败。
#[test]
fn goal_pasted_objective_survives_submission_and_continuation() {
    let body = format!(
        "原始需求\n{}\n    保留  缩进\n完成测试",
        "详细验收条件\n".repeat(30)
    );
    let mut clipboard = ReplClipboardState::default();
    let mut input = "/goal ".to_string();
    let mut cursor = input.chars().count();
    assert!(clipboard.paste_text_into_input(&mut input, &mut cursor, body.clone()));
    input.push_str(" --tokens 5000");
    let submission = ReplInputSubmission::from_input(AgentMode::Yolo, input.clone(), &clipboard);
    clipboard.clear();
    assert_eq!(submission.raw_input, input);
    assert_eq!(submission.history.text, input);
    assert_goal_round_trip(&submission, &body);
}

/// 【目标】【提交测试】整条命令粘贴和控制队列提交使用相同完整正文。
/// 参数: 无；返回: 无，命令分类或正文发生变化时断言失败。
#[test]
fn goal_whole_command_paste_and_control_submission_keep_objective() {
    let body = "检查所有验收条件\n".repeat(30).trim().to_string();
    let command = format!("/goal {body} --tokens=5000");
    let mut clipboard = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    clipboard.paste_text_into_input(&mut input, &mut cursor, command.clone());
    let pasted = ReplInputSubmission::from_input(AgentMode::Yolo, input, &clipboard);
    assert_goal_round_trip(&pasted, &body);
    assert_goal_round_trip(
        &ReplInputSubmission::control(AgentMode::Yolo, command),
        &body,
    );
}

/// 【目标】【提交测试】编辑目标时多段粘贴仍按输入顺序展开，不追加聊天包装。
/// 参数: 无；返回: 无，原位展开失效时断言失败。
#[test]
fn goal_edit_expands_multiple_blocks_in_place() {
    let mut clipboard = ReplClipboardState::default();
    let mut input = "/goal edit ".to_string();
    let first = "第一部分需求\n".repeat(30).trim().to_string();
    let second = "第二部分需求\n".repeat(30).trim().to_string();
    let mut cursor = input.chars().count();
    clipboard.paste_text_into_input(&mut input, &mut cursor, first.clone());
    input.push_str("\n中间说明\n");
    cursor = input.chars().count();
    clipboard.paste_text_into_input(&mut input, &mut cursor, second.clone());
    let submission = ReplInputSubmission::from_input(AgentMode::Yolo, input, &clipboard);
    let Some(ControlCommand::Goal(crate::control_commands::GoalCommand::Edit { objective })) =
        parse_control_command(submission.command_input(), ControlSurface::Repl).unwrap()
    else {
        panic!("提交没有解析为目标编辑命令");
    };
    assert_eq!(objective, format!("{first}\n中间说明\n{second}"));
}
