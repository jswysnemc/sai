use super::*;
use crate::cli::repl_text::visible_width;

/// 【终端】【agent 面板】条目行同时给出身份、当前动作与进度。
///
/// 此前右栏只有一个「Delegating + 描述」，同屏几个子智能体时
/// 除了描述本身没有任何可区分信息。
#[test]
fn entry_title_combines_identity_and_progress() {
    let entry = SubagentOverviewEntry {
        cell_index: 0,
        label: "诗歌文本多阶段分析".to_string(),
        status: "run",
        running: true,
        viewing: false,
        detail: Some("Reading foo.rs".to_string()),
        tokens: Some(12_300),
        agent_type: Some("explore".to_string()),
        progress: Some((3, 20)),
        elapsed_seconds: Some(134),
    };

    assert_eq!(
        render_entry_title(&entry),
        "explore 诗歌文本多阶段分析 · 3/20 · 2m14s · 12k tokens"
    );
}

/// 缺失的可选字段整段省略，不留下空的分隔符。
#[test]
fn entry_title_omits_missing_segments() {
    let entry = SubagentOverviewEntry {
        cell_index: 0,
        label: "查资料".to_string(),
        status: "run",
        running: true,
        viewing: false,
        detail: None,
        tokens: None,
        agent_type: None,
        progress: None,
        elapsed_seconds: None,
    };

    assert_eq!(render_entry_title(&entry), "查资料");
}

/// 时长在秒 / 分 / 小时之间正确进位。
#[test]
fn elapsed_is_formatted_compactly() {
    assert_eq!(format_elapsed(42), "42s");
    assert_eq!(format_elapsed(134), "2m14s");
    assert_eq!(format_elapsed(7_500), "2h05m");
}

/// 构造测试概览条目。
fn entry(cell_index: usize, label: &str, running: bool) -> SubagentOverviewEntry {
    SubagentOverviewEntry {
        cell_index,
        label: label.to_string(),
        status: if running { "run" } else { "ok" },
        running,
        viewing: false,
        detail: None,
        tokens: None,
        agent_type: None,
        progress: None,
        elapsed_seconds: None,
    }
}

#[test]
fn activate_requires_subagents() {
    let mut panel = AgentPanelState::default();
    assert!(!panel.activate(&[]));
    assert!(panel.activate(&[entry(2, "检查项目", true)]));
    assert!(panel.is_active());
}

#[test]
fn keys_cycle_and_apply_selection() {
    let mut panel = AgentPanelState::default();
    let entries = vec![entry(2, "one", true), entry(5, "two", false)];
    panel.activate(&entries);
    assert_eq!(
        panel.handle_key(KeyCode::Down, &entries),
        AgentPanelAction::Consumed
    );
    assert_eq!(
        panel.handle_key(KeyCode::Down, &entries),
        AgentPanelAction::Consumed
    );
    // 主(0) → one(1) → two(2)，Enter 应返回 two 的 cell_index
    assert_eq!(
        panel.handle_key(KeyCode::Enter, &entries),
        AgentPanelAction::Apply(Some(5))
    );
    assert!(!panel.is_active());
}

#[test]
fn enter_on_main_returns_none() {
    let mut panel = AgentPanelState::default();
    let entries = vec![entry(2, "one", true)];
    panel.activate(&entries);
    assert_eq!(
        panel.handle_key(KeyCode::Enter, &entries),
        AgentPanelAction::Apply(None)
    );
}

#[test]
fn escape_and_other_keys_leave_panel() {
    let mut panel = AgentPanelState::default();
    let entries = vec![entry(2, "one", true)];
    panel.activate(&entries);
    assert_eq!(
        panel.handle_key(KeyCode::Esc, &entries),
        AgentPanelAction::Exit
    );
    assert!(!panel.is_active());
    panel.activate(&entries);
    assert_eq!(
        panel.handle_key(KeyCode::Char('x'), &entries),
        AgentPanelAction::Ignored
    );
    assert!(!panel.is_active());
}

/// ↑ 回到输入框：主智能体项再按 ↑ 收成单行，而不是绕回最后一项。
#[test]
fn up_on_the_main_entry_returns_to_the_input_box() {
    let mut panel = AgentPanelState::default();
    let entries = vec![entry(2, "one", true), entry(5, "two", false)];
    panel.activate(&entries);
    assert_eq!(
        panel.handle_key(KeyCode::Up, &entries),
        AgentPanelAction::Exit
    );
    assert!(!panel.is_active());
    // 收起后只剩一行摘要
    assert_eq!(panel.panel_lines(&entries, 0).len(), 1);
    // 选中项已越过主项时，↑ 先退回上一项
    panel.activate(&entries);
    panel.handle_key(KeyCode::Down, &entries);
    panel.handle_key(KeyCode::Down, &entries);
    assert_eq!(
        panel.handle_key(KeyCode::Up, &entries),
        AgentPanelAction::Consumed
    );
    assert!(panel.is_active());
}

/// 非焦点态只占一行：底栏还要留给 todo 与消息队列，铺开会把输入框顶上去。
#[test]
fn inactive_panel_is_a_single_summary_line() {
    let panel = AgentPanelState::default();
    let entries = vec![entry(2, "one", true), entry(5, "two", false)];
    let lines = panel.panel_lines(&entries, 0);
    assert_eq!(lines.len(), 1);
    let plain = crate::render::activity_animation::strip_ansi_for_test(&lines[0]);
    assert!(plain.starts_with('●'), "摘要行应以脉冲圆点起头: {plain}");
    assert!(plain.contains("(2)"), "摘要行应给出条目数: {plain}");
    assert!(plain.contains('1'), "摘要行应给出运行中条数: {plain}");
    assert!(plain.contains('↓'), "摘要行应提示 ↓ 展开: {plain}");
    // 条目细节留给 ↓ 展开，不在摘要行里占位
    assert!(!plain.contains("one"), "{plain}");
    assert!(!plain.contains("two"), "{plain}");
}

/// 子 agent 再多，收起态也只占一行。
#[test]
fn inactive_panel_height_does_not_grow_with_entry_count() {
    let panel = AgentPanelState::default();
    let entries: Vec<SubagentOverviewEntry> = (0..8)
        .map(|index| entry(index, &format!("任务{index}"), true))
        .collect();
    assert_eq!(panel.panel_lines(&entries, 0).len(), 1);
    let single = panel.panel_lines(&entries[..1], 0);
    assert_eq!(single.len(), 1);
}

#[test]
fn active_panel_lists_main_and_subagents() {
    let mut panel = AgentPanelState::default();
    let entries = vec![entry(2, "检查项目", true)];
    panel.activate(&entries);
    let lines = panel.panel_lines(&entries, 0);
    assert!(lines.len() >= 3);
    assert!(lines.iter().any(|line| line.contains('❯')));
    assert!(lines.iter().any(|line| line.contains("检查项目")));
}

/// 【终端】【子任务面板】运行状态与用量同时可见，查看中的任务有独立标记。
#[test]
fn active_panel_shows_token_cell_and_viewing_marker() {
    let mut panel = AgentPanelState::default();
    let mut running_entry = entry(2, "诗歌分析", true);
    running_entry.tokens = Some(1_200);
    let mut idle_entry = entry(5, "长期重构", false);
    idle_entry.status = "idle";
    idle_entry.viewing = true;
    let entries = vec![running_entry, idle_entry];
    panel.activate(&entries);

    let lines = panel.panel_lines(&entries, 0);
    let joined = lines.join("\n");
    let plain = crate::render::activity_animation::strip_ansi_for_test(&joined);
    assert!(plain.contains("1.2k"), "运行中条目应保留用量: {plain}");
    assert!(
        plain.contains(t("Idle", "待命")),
        "待命且无 tokens 的条目左栏应展示 idle: {joined}"
    );
    assert!(joined.contains("查看中") || joined.contains("viewing"));
    assert!(
        !joined.contains("消耗 Token"),
        "长阶段文案不应再挤进标题栏: {joined}"
    );
}

/// 【终端】【agent 面板】标题行顶格带引导点，条目行缩进且标题与 tokens 分列对齐。
#[test]
fn header_is_a_guide_dot_and_entry_columns_align() {
    let mut panel = AgentPanelState::default();
    let mut running_entry = entry(2, "诗歌分析", true);
    running_entry.tokens = Some(1_200);
    let mut idle_entry = entry(5, "长期重构", false);
    idle_entry.status = "idle";
    idle_entry.tokens = Some(12);
    let entries = vec![running_entry, idle_entry];
    panel.activate(&entries);

    let lines = panel.panel_lines(&entries, 0);
    let plain_lines: Vec<String> = lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line))
        .collect();
    // 标题行与 todo、队列同一套沉底装饰：顶格引导点
    assert!(
        plain_lines[0].starts_with('●'),
        "标题行应以引导点顶格: {:?}",
        plain_lines[0]
    );
    let rows = &plain_lines[1..];
    let starts: Vec<usize> = rows
        .iter()
        .zip([t("main agent", "主智能体"), "诗歌分析", "长期重构"])
        .map(|(line, title)| {
            let byte_index = line
                .find(title)
                .unwrap_or_else(|| panic!("missing {title} in {line}"));
            visible_width(&line[..byte_index])
        })
        .collect();
    assert_eq!(
        starts.len(),
        3,
        "主智能体与两条条目都应出现: {plain_lines:?}"
    );
    assert!(
        starts.windows(2).all(|pair| pair[0] == pair[1]),
        "条目标题栏起点必须对齐: {starts:?} / {plain_lines:?}"
    );
    assert!(rows[1].contains(t("Running", "运行")));
    assert!(rows[2].contains(t("Idle", "待命")));
    assert!(rows[1].contains("1.2k tokens") && rows[2].contains("12 tokens"));
}

/// 大量子任务展开后仍使用固定窗口，选中项始终可见。
#[test]
fn active_panel_window_keeps_selected_task_visible() {
    let mut panel = AgentPanelState::default();
    let entries = (0..30)
        .map(|index| entry(index, &format!("task-{index}"), true))
        .collect::<Vec<_>>();
    panel.activate(&entries);
    for _ in 0..26 {
        panel.handle_key(KeyCode::Down, &entries);
    }
    let lines = panel.panel_lines(&entries, 0);
    assert!(lines.len() <= 8);
    assert!(lines.iter().any(|line| line.contains("task-25")));
}

/// 中断任务有独立状态和摘要，不显示完成色或运行动效。
#[test]
fn interrupted_subagent_panel_stays_neutral() {
    let mut stopped = entry(2, "检查项目", false);
    stopped.status = "interrupted";
    let entries = vec![stopped];
    let summary = idle_line(&entries, 4);
    assert!(summary.contains(t("interrupted", "已中断")));
    assert!(!summary.contains(t("completed", "已完成")));
    assert!(!summary.contains(t("running", "运行中")));
    let left = render_entry_left(&entries[0], status_column_width(&entries), 4);
    assert!(left.contains(t("Interrupted", "已中断")));
    assert!(!left.contains("\x1b[32m"));
    assert_eq!(
        left,
        render_entry_left(&entries[0], status_column_width(&entries), 12)
    );
}
