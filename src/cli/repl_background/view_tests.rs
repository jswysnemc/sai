use super::{
    input,
    model::{Filter, Output, TaskManager},
    model_tests::task,
    panel_rect::PanelRect,
    service::{Request, Update},
    split_layout::SplitLayout,
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
    assert!(
        row("任务 run").contains(crate::i18n::text(" running ", " 运行中")),
        "{text}"
    );
    assert!(
        row("任务 done").contains(crate::i18n::text(" stopped ", " 已停止")),
        "{text}"
    );
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

/// 【后台任务测试】【响应式布局】宽面板自动使用左右双栏，窄或矮面板切换为上下堆叠且保证视口边界。
/// 参数: 无；返回: 无。
#[test]
fn list_layout_adapts_split_on_wide_and_stacked_on_narrow() {
    assert!(matches!(
        SplitLayout::compute(96, 24, 5, false),
        SplitLayout::Split {
            left_w: 40,
            right_w: 53,
            task_rows: 22,
            detail_rows: 4,
            tail_rows: 17,
        }
    ));
    assert_eq!(
        SplitLayout::compute(70, 29, 5, false),
        SplitLayout::Stacked {
            task_rows: 5,
            tail_rows: 18,
        }
    );
    assert_eq!(
        SplitLayout::compute(70, 29, 50, false),
        SplitLayout::Stacked {
            task_rows: 15,
            tail_rows: 8,
        }
    );
    assert_eq!(
        SplitLayout::compute(70, 10, 5, false),
        SplitLayout::Stacked {
            task_rows: 4,
            tail_rows: 0,
        }
    );
    assert_eq!(
        SplitLayout::compute(70, 3, 5, false),
        SplitLayout::Stacked {
            task_rows: 1,
            tail_rows: 0,
        }
    );
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

/// 【后台任务测试】【命令语法着色】详情卡、停止确认卡与无标签队列行均输出 ANSI 着色且保留纯文本命令。
/// 参数: 无；返回: 无。
#[test]
fn details_and_unlabeled_row_render_syntax_colored_commands() {
    let mut manager = TaskManager::default();
    let mut unlabeled = task("t1", "running");
    unlabeled.label.clear();
    unlabeled.command = "echo 'hello' && sleep 5".into();
    manager.tasks = vec![unlabeled];
    manager.loading = false;
    manager.reconcile();

    let frame = with_locale(Locale::En, || view::render(&mut manager, rect((110, 24))));
    let cmd_line = frame
        .lines
        .iter()
        .find(|line| strip_ansi(line).contains("Command"))
        .expect("command detail line");
    let plain_cmd = strip_ansi(cmd_line);
    assert!(
        plain_cmd.contains("$ echo 'hello' && sleep 5"),
        "{plain_cmd}"
    );
    let echo_pos = cmd_line.find("echo").expect("command token present");
    let sleep_end = cmd_line.find("sleep").expect("sleep token present");
    assert!(
        cmd_line[echo_pos..sleep_end].contains("\x1b[38;"),
        "expected syntax foreground SGR between command tokens: {cmd_line:?}"
    );

    let list_line = frame
        .lines
        .iter()
        .find(|line| {
            let stripped = strip_ansi(line);
            stripped.contains("▎") && stripped.contains("echo 'hello'")
        })
        .expect("unlabeled task row falls back to colored command");
    assert!(list_line.contains("\x1b[48;2;"), "{list_line:?}");

    manager.confirm = Some(Request::Stop {
        id: "t1".into(),
        force: false,
    });
    let confirm_frame = with_locale(Locale::En, || view::render(&mut manager, rect((110, 24))));
    let confirm_cmd = confirm_frame
        .lines
        .iter()
        .find(|line| strip_ansi(line).contains("Command"))
        .expect("confirm command line");
    assert!(strip_ansi(confirm_cmd).contains("$ echo 'hello' && sleep 5"));
    let confirm_echo = confirm_cmd.find("echo").expect("confirm echo token");
    let confirm_sleep = confirm_cmd.find("sleep").expect("confirm sleep token");
    assert!(confirm_cmd[confirm_echo..confirm_sleep].contains("\x1b[38;"));
}

/// 【后台任务测试】【命令展开与边界】按 c 展开被截断的多行命令，并在各终端尺寸下严守列宽行高上限。
/// 参数: 无；返回: 无。
#[test]
fn command_expand_key_reveals_truncated_multiline_command_within_bounds() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut manager = TaskManager::default();
    let mut item = task("long", "running");
    item.command = concat!(
        "export DATABASE_URL='postgres://localhost:5432/app'\n",
        "cargo run --release --bin worker -- --concurrency 8 --queue priority_jobs\n",
        "echo 'FINAL_SENTINEL_MARKER'"
    )
    .into();
    manager.tasks = vec![item];
    manager.loading = false;
    manager.reconcile();

    let collapsed = plain(&mut manager, (100, 24)).join("\n");
    assert!(collapsed.contains("[c] Expand cmd"), "{collapsed}");
    assert!(collapsed.contains("c command"), "{collapsed}");
    assert!(
        !collapsed.contains("FINAL_SENTINEL_MARKER"),
        "single-line preview should truncate multiline tail: {collapsed}"
    );

    input::handle(
        &mut manager,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        10,
    );
    assert!(manager.command_expanded);

    let expanded = plain(&mut manager, (100, 24)).join("\n");
    assert!(expanded.contains("[c] Collapse cmd"), "{expanded}");
    assert!(expanded.contains("c collapse"), "{expanded}");
    assert!(expanded.contains("DATABASE_URL"), "{expanded}");
    assert!(expanded.contains("priority_jobs"), "{expanded}");
    assert!(expanded.contains("FINAL_SENTINEL_MARKER"), "{expanded}");

    for size in [(96, 20), (60, 16), (20, 8), (2, 2), (0, 0)] {
        for locale in [Locale::En, Locale::Zh] {
            let frame = with_locale(locale, || view::render(&mut manager, rect(size)));
            assert!(frame.lines.len() <= usize::from(size.1), "{size:?}");
            for line in &frame.placed() {
                assert!(
                    ansi_visible_width(line) <= usize::from(size.0),
                    "{size:?}: {line}"
                );
            }
        }
    }
}
