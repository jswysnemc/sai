use super::{
    input,
    list_body::ListLayout,
    model::{Filter, Output, TaskManager},
    model_tests::task,
    panel_rect::PanelRect,
    service::{Request, Update},
    view,
};
use crate::i18n::{with_locale, Locale};
use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};

/// 【后台任务测试】【面板矩形】按独立管理屏幕的方式占满整个终端。
/// 参数: size 为终端列数和行数；返回: 面板矩形。
fn rect(size: (u16, u16)) -> PanelRect {
    PanelRect::inset(size.0, 0, size.1)
}

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
        // 1. 【后台任务测试】【双语宽度】内联视图沿用中文界面，双宽标签同样不能越界
        for (pager, locale) in [false, true]
            .into_iter()
            .flat_map(|pager| [(pager, Locale::En), (pager, Locale::Zh)])
        {
            manager.pager = pager;
            let frame = with_locale(locale, || view::render(&mut manager, rect(size)));
            assert!(frame.lines.len() <= usize::from(size.1), "{size:?}");
            for line in &frame.placed() {
                assert!(
                    ansi_visible_width(line) <= usize::from(size.0),
                    "{size:?}: {line}"
                );
            }
            // 2. 【后台任务测试】【有限视口】任务行不超过任务视口高度
            assert!(frame.targets.len() <= frame.page_rows.max(1), "{size:?}");
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
    let text = strip_ansi(&view::render(&mut manager, rect((100, 24))).lines.join("\n"));
    let row = |label: &str| {
        text.lines()
            .find(|line| line.contains(label))
            .unwrap_or_default()
            .to_string()
    };
    assert!(row("任务 run").contains(" running "), "{text}");
    assert!(row("任务 done").contains(" stopped "), "{text}");
    assert!(text.contains("/tmp/项目"));
    assert!(text.contains("echo hello"));
    manager.confirm = Some(Request::Stop {
        id: "run".into(),
        force: true,
    });
    let text = strip_ansi(&view::render(&mut manager, rect((100, 24))).lines.join("\n"));
    assert!(text.contains("run"));
    assert!(text.contains("Enter/y"));
    assert!(text.contains("Esc/n"));
    assert_eq!(view::duration(325), "5m 25s");
}

/// 【后台任务测试】【面板样本】一个运行中任务加两个已结束任务，选中运行中任务。
/// 参数: 无；返回: 已整理选择的面板状态。
fn sample() -> TaskManager {
    let mut manager = TaskManager::default();
    manager.tasks = vec![
        task("run", "running"),
        task("fail", "failed"),
        task("done", "exited"),
    ];
    manager.loading = false;
    manager.reconcile();
    manager
}

/// 【后台任务测试】【纯文本帧】按英文界面绘制并去除样式。
/// 参数: manager 为面板状态，size 为终端尺寸；返回: 逐行纯文本。
fn plain(manager: &mut TaskManager, size: (u16, u16)) -> Vec<String> {
    with_locale(Locale::En, || view::render(manager, rect(size)).lines)
        .iter()
        .map(|line| strip_ansi(line))
        .collect()
}

/// 【后台任务测试】【筛选稳定】切换筛选范围时各标签列位置不变。
/// 参数: 无；返回: 无。
#[test]
fn filter_tabs_keep_columns_when_switching() {
    let mut manager = sample();
    let mut columns = Vec::new();
    for filter in [Filter::All, Filter::Running, Filter::Finished] {
        manager.set_filter(filter);
        let lines = plain(&mut manager, (100, 24));
        let row = lines.iter().find(|line| line.contains("1 All")).unwrap();
        columns.push((row.find("Running").unwrap(), row.find("Finished").unwrap()));
    }
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "{columns:?}"
    );
}

/// 【后台任务测试】【页脚按键】停止只对运行中任务显示，窄面板整段舍弃且不截断按键名称。
/// 参数: 无；返回: 无。
#[test]
fn footer_lists_only_available_actions_and_drops_whole_segments() {
    let mut manager = sample();
    let footer = |manager: &mut TaskManager, size| {
        plain(manager, size)
            .iter()
            .rev()
            .find(|line| line.contains('╰'))
            .cloned()
            .unwrap()
    };
    assert!(footer(&mut manager, (100, 24)).contains("s stop · f force"));
    manager.move_selection(1);
    let finished = footer(&mut manager, (100, 24));
    assert!(
        !finished.contains("stop") && finished.contains("x clean"),
        "{finished}"
    );
    let narrow = footer(&mut manager, (40, 24));
    assert!(!narrow.contains('…'), "{narrow}");
    assert!(narrow.contains("Esc back"), "{narrow}");
}

/// 【后台任务测试】【确认上下文】停止确认保留任务列表与面板高度，详情区换成目标信息。
/// 参数: 无；返回: 无。
#[test]
fn confirm_keeps_task_list_and_panel_height() {
    let mut manager = sample();
    let before = plain(&mut manager, (100, 24));
    manager.confirm = Some(Request::Stop {
        id: "run".into(),
        force: false,
    });
    let after = plain(&mut manager, (100, 24));
    assert_eq!(before.len(), after.len());
    let text = after.join("\n");
    assert!(
        text.contains("任务 fail") && text.contains("Stop run"),
        "{text}"
    );
    assert!(!text.contains("Details"), "{text}");
    assert!(view::render(&mut manager, rect((100, 24)))
        .targets
        .is_empty());
}

/// 【后台任务测试】【日志滑块】日志溢出时右边框出现粗线滑块，未溢出时不显示。
/// 参数: 无；返回: 无。
#[test]
fn pager_border_shows_thumb_only_when_logs_overflow() {
    let mut manager = sample();
    manager.pager = true;
    manager.output_id = manager.selected_id.clone();
    manager.output.stdout = Some("short\n".into());
    assert!(!plain(&mut manager, (100, 24)).join("\n").contains('┃'));
    manager.output.stdout = Some((0..80).map(|index| format!("line {index}\n")).collect());
    let thumbs = plain(&mut manager, (100, 24))
        .iter()
        .filter(|line| line.ends_with('┃'))
        .count();
    assert!((1..14).contains(&thumbs), "{thumbs}");
    assert_eq!(view::thumb(66, 14, 80), 12..14);
}

/// 【后台任务测试】【悬停反馈】指针经过任务行时记录悬停目标，离开面板后清除。
/// 参数: 无；返回: 无。
#[test]
fn pointer_motion_tracks_hovered_task_row() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let mut manager = sample();
    let frame = view::render(&mut manager, rect((100, 24)));
    let (row, id) = frame.targets[1].clone();
    let moved = |column, row| MouseEvent {
        kind: MouseEventKind::Moved,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    input::mouse(&mut manager, moved(frame.rect.x + 4, row), &frame);
    assert_eq!(manager.hovered.as_deref(), Some(id.as_str()));
    input::mouse(&mut manager, moved(0, row), &frame);
    assert_eq!(manager.hovered, None);
}

/// 【后台任务测试】【高度分配】任务少时视口贴合任务数，任务多时至多占可变行三分之二，矮面板省略输出区。
/// 参数: 无；返回: 无。
#[test]
fn list_layout_gives_spare_rows_to_output_tail() {
    let layout = |rows, total| {
        let layout = ListLayout::compute(rows, total);
        (layout.tasks, layout.output)
    };
    assert_eq!(layout(29, 5), (5, 18));
    assert_eq!(layout(29, 50), (15, 8));
    assert_eq!(layout(29, 0), (1, 22));
    assert_eq!(layout(13, 5), (4, 3));
    assert_eq!(layout(10, 5), (4, 0));
    assert_eq!(layout(3, 5), (1, 0));
}

/// 【后台任务测试】【尾部输出】列表模式显示选中任务最后几行日志及行号，高度随面板增长。
/// 参数: 无；返回: 无。
#[test]
fn list_shows_output_tail_sized_by_panel_height() {
    let mut manager = sample();
    manager.output_id = manager.selected_id.clone();
    manager.output.stdout = Some((1..=40).map(|index| format!("line {index}\n")).collect());
    let tail = |manager: &mut TaskManager, rows| {
        plain(manager, (100, rows))
            .into_iter()
            .filter(|line| line.contains("│ line "))
            .count()
    };
    let small = tail(&mut manager, 20);
    let large = tail(&mut manager, 40);
    assert!(small >= 3 && large > small + 15, "{small} {large}");
    let text = plain(&mut manager, (100, 40)).join("\n");
    assert!(text.contains("Output · STDOUT · 40 lines"), "{text}");
    assert!(
        text.contains("   40 │ line 40") && !text.contains("    1 │ line 1 "),
        "{text}"
    );
}

/// 【后台任务测试】【面板外滚轮】面板矩形之外的滚轮不移动选择，面板内的滚轮正常移动。
/// 参数: 无；返回: 无。
#[test]
fn wheel_outside_panel_rect_is_ignored() {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    let mut manager = sample();
    let frame = view::render(&mut manager, PanelRect::inset(100, 2, 20));
    let wheel = |row| MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row,
        modifiers: KeyModifiers::NONE,
    };
    input::mouse(&mut manager, wheel(23), &frame);
    assert_eq!(manager.selected_id.as_deref(), Some("run"));
    input::mouse(&mut manager, wheel(10), &frame);
    assert_eq!(manager.selected_id.as_deref(), Some("done"));
}
