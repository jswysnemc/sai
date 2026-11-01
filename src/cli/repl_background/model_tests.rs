use super::{
    input::{handle, Effect},
    model::{Filter, Output, Stream, Task, TaskManager},
    service::{Request, Update},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 【后台任务测试】【任务样本】创建不启动进程的任务。
/// 参数: id/status 为标识与状态；返回: 测试条目。
pub(super) fn task(id: &str, status: &str) -> Task {
    Task {
        id: id.into(),
        label: format!("任务 {id}"),
        command: "echo hello".into(),
        cwd: "/tmp/项目".into(),
        pid: 123,
        status: status.into(),
        started_at: 10,
        updated_at: 70,
        timeout_seconds: 0,
    }
}
/// 【后台任务测试】【按键样本】提交无修饰按键。
/// 参数: state 为状态，key 为按键；返回: 操作效果。
fn press(state: &mut TaskManager, key: KeyCode) -> Effect {
    handle(state, KeyEvent::new(key, KeyModifiers::NONE), 10)
}

/// 【后台任务测试】【稳定选择】刷新排序不改变已有选择，删除和筛选修复选择。
/// 参数: 无；返回: 无。
#[test]
fn selection_survives_refresh_and_filters() {
    let mut state = TaskManager::default();
    state.apply(Update {
        tasks: vec![task("done", "exited"), task("run", "running")],
        output: None,
        notice: None,
    });
    assert_eq!(state.selected_id.as_deref(), Some("run"));
    press(&mut state, KeyCode::Down);
    assert_eq!(state.selected_id.as_deref(), Some("done"));
    state.apply(Update {
        tasks: vec![
            task("new", "running"),
            task("run", "running"),
            task("done", "exited"),
        ],
        output: None,
        notice: None,
    });
    assert_eq!(state.selected_id.as_deref(), Some("done"));
    state.set_filter(Filter::Running);
    assert_eq!(state.selected_id.as_deref(), Some("new"));
    state.apply(Update {
        tasks: vec![],
        output: None,
        notice: None,
    });
    assert!(state.selected().is_none());
}

/// 【后台任务测试】【尾随滚动】暂停从当前尾页开始，首尾导航与通道切换保持边界。
/// 参数: 无；返回: 无。
#[test]
fn pager_starts_scroll_from_current_tail() {
    let mut state = TaskManager::default();
    state.apply(Update {
        tasks: vec![task("run", "running")],
        output: Some((
            "run".into(),
            Ok(Output {
                stdout: Some(
                    (0..100)
                        .map(|i| format!("line {i}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                ..Output::default()
            }),
        )),
        notice: None,
    });
    press(&mut state, KeyCode::Enter);
    assert!(state.pager);
    assert_eq!(state.pager_offset(10), 90);
    press(&mut state, KeyCode::Up);
    assert_eq!(state.offset, 89);
    assert!(!state.follow);
    press(&mut state, KeyCode::Char('g'));
    assert_eq!(state.offset, 0);
    press(&mut state, KeyCode::Char('G'));
    assert_eq!(state.offset, 90);
    press(&mut state, KeyCode::Char('f'));
    assert!(state.follow);
    press(&mut state, KeyCode::Tab);
    assert_eq!(state.stream, Stream::Stderr);
    assert_eq!(state.pager_offset(10), 0);
    press(&mut state, KeyCode::Esc);
    assert!(!state.pager);
    assert_eq!(press(&mut state, KeyCode::Esc), Effect::Exit);
}

/// 【后台任务测试】【停止确认】普通与强制停止需要确认，取消不执行，终态不可停止。
/// 参数: 无；返回: 无。
#[test]
fn destructive_actions_require_confirmation() {
    let mut state = TaskManager::default();
    state.tasks = vec![task("run", "running"), task("done", "exited")];
    state.reconcile();
    assert_eq!(press(&mut state, KeyCode::Char('s')), Effect::None);
    assert_eq!(
        state.confirm,
        Some(Request::Stop {
            id: "run".into(),
            force: false
        })
    );
    press(&mut state, KeyCode::Esc);
    assert!(state.confirm.is_none());
    press(&mut state, KeyCode::Char('f'));
    assert_eq!(
        press(&mut state, KeyCode::Enter),
        Effect::Request(Request::Stop {
            id: "run".into(),
            force: true
        })
    );
    state.set_filter(Filter::Finished);
    press(&mut state, KeyCode::Char('s'));
    assert!(state.confirm.is_none());
    press(&mut state, KeyCode::Char('x'));
    assert_eq!(
        press(&mut state, KeyCode::Char('y')),
        Effect::Request(Request::Cleanup)
    );
}

/// 【后台任务测试】【输出隔离】迟到结果不覆盖新选择，日志不执行终端控制序列。
/// 参数: 无；返回: 无。
#[test]
fn output_is_bound_to_task_and_sanitized() {
    let mut state = TaskManager::default();
    state.tasks = vec![task("A", "running"), task("B", "running")];
    state.selected_id = Some("B".into());
    state.apply(Update {
        tasks: state.tasks.clone(),
        output: Some(("A".into(), Ok(Output::default()))),
        notice: None,
    });
    assert!(state.output_id.is_none());
    state.output_id = Some("B".into());
    state.output.stdout = Some("\x1b[2Jhello\x1b]52;c;bad\x07\nworld".into());
    state.output.stderr = Some("error".into());
    state.stream = Stream::All;
    assert_eq!(
        state.log_lines(),
        vec!["[out] hello", "[out] world", "[err] error"]
    );
}
