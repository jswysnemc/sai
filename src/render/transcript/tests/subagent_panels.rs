use super::*;

#[test]
fn subagent_view_switch_replaces_display_window() {
    let mut store = TranscriptStore::new(100);
    store.push_meta("主会话内容".to_string());
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"检查项目"}"#.to_string(),
    );
    // 绑定后台 ID：running 状态下 finish 只记录 ID
    store.push_tool_result(
        "subagent".to_string(),
        true,
        r#"{"subagent":{"id":"sub-test-1","status":"running"}}"#.to_string(),
    );

    // 1. 进入子智能体视图：窗口内容替换为其会话时间线
    assert!(store.enter_subagent_view(1));
    assert_eq!(store.viewing_subagent_id(), Some("sub-test-1"));
    let view = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    // 子智能体视图标题与主视图工具行同语汇（Delegating/Delegated）
    assert!(view.contains("Delegating") || view.contains("Delegated"));
    assert!(!view.contains("主会话内容"));

    // 2. 返回主视图：恢复主会话内容
    assert!(store.exit_subagent_view());
    assert!(!store.exit_subagent_view());
    let main = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(main.contains("主会话内容"));
}

/// 【终端】【agent 面板】已结束的子智能体保留在面板里。
///
/// 并发跑完一批子智能体后，用户要能在一处看到每个任务的结果与最终
/// 用量；此前终态条目会直接从列表消失，跑完就再也查不到消耗。
#[test]
fn subagent_overview_keeps_finished_entries() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"完成的"}"#.to_string(),
    );
    store.push_tool_result("subagent".to_string(), true, "plain result".to_string());
    let overview = store.subagent_overview();
    assert_eq!(overview.len(), 1, "finished subagents stay in the panel");
    assert!(!overview[0].running);
    assert!(overview[0].label.contains("完成的"));
}

/// 【终端】【agent 面板】同一子智能体的多次工具调用只保留一个面板条目。
///
/// 主代理每次 subagent 调用（start / wait / send / result）都会产生
/// 一个 transcript cell，此前每个 cell 都各占一行导致面板大量重复。
#[test]
fn subagent_overview_deduplicates_repeated_calls_by_id() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "诗歌文本多阶段分析".to_string(),
        "explore".to_string(),
        3,
    );
    let bound_result = format!(
        r#"{{"subagent":{{"id":"{}","status":"running"}}}}"#,
        subagent.id
    );
    let mut store = TranscriptStore::new(100);
    // 同一个子智能体：start + 多次 wait/send，每次调用都是一个独立 cell
    for _ in 0..3 {
        store.push_tool_call(
            "subagent".to_string(),
            format!(r#"{{"action":"wait","id":"{}"}}"#, subagent.id),
        );
        store.push_tool_result("subagent".to_string(), true, bound_result.clone());
    }

    let overview = store.subagent_overview();

    assert_eq!(overview.len(), 1, "同一子智能体必须去重: {overview:?}");
    assert!(overview[0].running);
    assert_eq!(overview[0].status, "run");
}

/// 【终端】【agent 面板】尚未返回的 wait 调用按参数中的 subagent_id 归并。
///
/// 回归：子智能体 ID 此前只从工具输出解析，进行中的调用还没有输出，于是
/// 拿不到 ID 也就无法去重。并发等待四个子智能体时，四条 wait 会各占一行，
/// 和真正的委派条目混在一起。
#[test]
fn subagent_overview_merges_pending_waits_by_argument_id() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "并发等待".to_string(),
        "general".to_string(),
        3,
    );
    let mut store = TranscriptStore::new(100);
    // 1. start 已返回并绑定后台 ID
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"并发等待"}"#.to_string(),
    );
    store.push_tool_result(
        "subagent".to_string(),
        true,
        format!(
            r#"{{"subagent":{{"id":"{}","status":"running"}}}}"#,
            subagent.id
        ),
    );
    // 2. 三次仍在进行中的 wait，没有任何结果可供解析
    for _ in 0..3 {
        store.push_tool_call(
            "subagent".to_string(),
            format!(r#"{{"action":"wait","subagent_id":"{}"}}"#, subagent.id),
        );
    }

    let overview = store.subagent_overview();

    assert_eq!(
        overview.len(),
        1,
        "进行中的 wait 应归并到同一条目: {overview:?}"
    );
}

#[test]
fn background_subagent_cell_reads_persisted_timeline() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "检查项目".to_string(),
        "explore".to_string(),
        5,
    );
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"检查项目"}"#.to_string(),
    );
    store.push_tool_result(
        "subagent".to_string(),
        true,
        serde_json::json!({"ok":true,"subagent":subagent.clone()}).to_string(),
    );
    crate::tools::subagent_state::timeline_streaming_text(&subagent.id, "正在检查", true);

    assert!(store.has_running_subagents());
    let running_signature = store.subagent_signature();
    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(rendered.contains("检查项目"));

    crate::tools::subagent_state::finish_subagent(
        &subagent.id,
        "completed",
        Some("检查完成".to_string()),
        None,
        None,
    );
    assert!(!store.has_running_subagents());
    assert_ne!(store.subagent_signature(), running_signature);
}
