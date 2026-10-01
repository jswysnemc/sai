use super::*;
use crate::tools::subagent_state::{
    create_subagent_for_owner, create_subagent_for_owner_goal, finish_subagent, park_subagent,
};

/// 【退出确认测试】【后台命令】构造一条归属指定会话的后台命令记录。
/// @param id 为任务 ID；session 为会话；pid 为进程号；status 为状态
/// @returns 后台命令记录
fn task(id: &str, session: &str, pid: u32, status: &str) -> BackgroundCommandTask {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "runtime_owner_kind": "session",
        "runtime_owner_id": session,
        "label": format!("label-{id}"),
        "command": "sleep 100",
        "cwd": "/tmp",
        "pid": pid,
        "status": status,
        "stdout_log": "",
        "stderr_log": "",
        "started_at": 0,
        "updated_at": 0,
        "timeout_seconds": 0,
        "completion_notified": false
    }))
    .expect("background task fixture")
}

/// 【退出确认测试】【隔离目录】准备测试用路径与两个会话目录。
/// @returns (临时目录, 路径, 当前会话目录, 其他会话目录)
fn fixture() -> (
    tempfile::TempDir,
    SaiPaths,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    paths.create_dirs().unwrap();
    let current = temp.path().join("session-current");
    let other = temp.path().join("session-other");
    std::fs::create_dir_all(&current).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    (temp, paths, current, other)
}

/// 关闭当前会话时，不能把另一个并行会话的子智能体算进来。
#[test]
fn exit_check_stays_inside_the_current_session() {
    let (_temp, paths, current, other) = fixture();
    let (subagent, _cancel) = create_subagent_for_owner(
        &other.display().to_string(),
        "other session task".to_string(),
        "general".to_string(),
        4,
    );
    assert!(running_work(&paths, "current", &current).is_empty());
    assert_eq!(running_work(&paths, "other", &other).subagents.len(), 1);
    finish_subagent(&subagent.id, "completed", None, None, None);
}

/// 空闲待命的持久子智能体也会随退出结束，必须计入；已结束的不计入。
#[test]
fn idle_persistent_subagents_are_counted() {
    let (_temp, paths, current, _other) = fixture();
    let owner = current.display().to_string();
    let (persistent, _cancel) = create_subagent_for_owner_goal(
        &owner,
        None,
        "watcher".to_string(),
        "general".to_string(),
        4,
        true,
    );
    assert!(park_subagent(&persistent.id, None, None));
    let (done, _cancel_done) =
        create_subagent_for_owner(&owner, "done".to_string(), "general".to_string(), 4);
    finish_subagent(&done.id, "completed", None, None, None);
    let work = running_work(&paths, "current", &current);
    assert_eq!(
        work.subagents
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["watcher"]
    );
    finish_subagent(&persistent.id, "completed", None, None, None);
}

/// 记录为运行中但进程已经不在的后台命令不再触发确认；其他会话的命令不计入。
#[test]
fn only_live_commands_of_this_session_are_counted() {
    let (_temp, paths, current, _other) = fixture();
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let live = std::process::id();
    store
        .save(&[
            task("alive", "current", live, "running"),
            task("dead", "current", u32::MAX - 7, "running"),
            task("finished", "current", live, "completed"),
            task("foreign", "other", live, "running"),
        ])
        .unwrap();
    let work = running_work(&paths, "current", &current);
    assert_eq!(
        work.commands
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["alive"]
    );
}

/// 没有后台命令时不提供“保留后台命令”，选项值与解析一一对应，取消一律留在会话。
#[test]
fn exit_question_offers_only_achievable_choices() {
    let subagent_only = RunningWork {
        subagents: vec![WorkItem {
            id: "s".into(),
            label: "reviewer".into(),
        }],
        commands: Vec::new(),
    };
    let request = exit_question(&subagent_only);
    request.validate().unwrap();
    let values = request.questions[0]
        .options
        .iter()
        .map(|option| option.answer_value().to_string())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![CHOICE_STOP, CHOICE_STAY]);
    assert!(request.questions[0].question.contains("reviewer"));
    let with_command = RunningWork {
        commands: vec![WorkItem {
            id: "c".into(),
            label: "dev server".into(),
        }],
        ..subagent_only
    };
    let request = exit_question(&with_command);
    request.validate().unwrap();
    assert_eq!(request.questions[0].options.len(), 3);
    let answered = |value: &str| QuestionResponse::Answered(vec![vec![value.to_string()]]);
    assert_eq!(exit_choice(&answered(CHOICE_STOP)), ExitChoice::StopAndExit);
    assert_eq!(
        exit_choice(&answered(CHOICE_LEAVE)),
        ExitChoice::LeaveRunning
    );
    assert_eq!(exit_choice(&answered(CHOICE_STAY)), ExitChoice::Stay);
    assert_eq!(exit_choice(&QuestionResponse::Cancelled), ExitChoice::Stay);
    assert_eq!(
        exit_choice(&QuestionResponse::Unavailable("no tty".into())),
        ExitChoice::Stay
    );
}

/// 选择全部停止后子智能体被取消，不再出现在待确认列表里。
#[tokio::test]
async fn stopping_work_cancels_subagents() {
    let (_temp, paths, current, _other) = fixture();
    let owner = current.display().to_string();
    let (subagent, _cancel) =
        create_subagent_for_owner(&owner, "task".to_string(), "general".to_string(), 4);
    let work = running_work(&paths, "current", &current);
    let failures = stop_work(&paths, &AppConfig::default(), &current, &work).await;
    assert!(failures.is_empty(), "{failures:?}");
    assert!(running_work(&paths, "current", &current).is_empty());
    finish_subagent(&subagent.id, "completed", None, None, None);
}
