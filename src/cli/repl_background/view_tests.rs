use super::{
    model::{Output, TaskManager},
    model_tests::task,
    service::{Request, Update},
    view,
};
use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};

/// 【后台任务测试】【窄屏布局】各尺寸与模式不越界，任务再多也只显示有限视口。
/// 参数: 无；返回: 无。
#[test]
fn compact_frames_fit_all_terminal_sizes() {
    let mut manager = TaskManager::default();
    manager.apply(Update {
        tasks: (0..50)
            .map(|index| task(&index.to_string(), "running"))
            .collect(),
        output: Some((
            "0".into(),
            Ok(Output {
                stdout: Some("中文输出\nline two".into()),
                ..Output::default()
            }),
        )),
        notice: None,
    });
    for size in [
        (120, 40),
        (96, 20),
        (60, 16),
        (40, 12),
        (20, 8),
        (2, 2),
        (0, 0),
    ] {
        for pager in [false, true] {
            manager.pager = pager;
            let frame = view::render(&mut manager, size);
            assert!(frame.lines.len() <= usize::from(size.1), "{size:?}");
            for line in &frame.lines {
                assert!(
                    ansi_visible_width(line) <= usize::from(size.0),
                    "{size:?}: {line}"
                );
            }
            assert!(frame.targets.len() <= 7);
        }
    }
}

/// 【后台任务测试】【信息结构】列表显示状态和可读时间，详情及确认包含明确目标。
/// 参数: 无；返回: 无。
#[test]
fn frame_contains_status_details_and_confirm_target() {
    let mut manager = TaskManager::default();
    manager.tasks = vec![task("run", "running"), task("done", "stopped")];
    manager.reconcile();
    let text = strip_ansi(&view::render(&mut manager, (100, 24)).lines.join("\n"));
    assert!(text.contains("[RUN ]"));
    assert!(text.contains("[STOP]"));
    assert!(text.contains("/tmp/项目"));
    assert!(text.contains("echo hello"));
    manager.confirm = Some(Request::Stop {
        id: "run".into(),
        force: true,
    });
    let text = strip_ansi(&view::render(&mut manager, (100, 24)).lines.join("\n"));
    assert!(text.contains("run"));
    assert!(text.contains("Enter/y"));
    assert!(text.contains("Esc/n"));
    assert_eq!(view::duration(325), "5m 25s");
}
