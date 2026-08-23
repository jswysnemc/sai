use super::test_support::{chunk, options};
use super::TranscriptStore;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::{strip_ansi_for_test, ACTIVITY_FRAME_INTERVAL};
use crate::render::work_status::WorkStatus;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use unicode_width::UnicodeWidthStr;

/// 【终端】【工作状态测试】验证实时思考正文与扫光标题同步渲染。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn live_reasoning_body_animates_without_waiting_for_consolidation() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(
        ChatStreamKind::Reasoning,
        "inspect resize\ncompare layout\n",
    ));

    let first = store.display_live_tail(80, &options());
    // 帧号由真实时间推导，直接把计时起点回拨来模拟动效推进。
    // 亮带起步于文字左侧的留白，回拨量需大到足以让它扫进文字区
    assert!(store.advance_live_animation());
    store.rewind_live_animation_for_test(ACTIVITY_FRAME_INTERVAL * 16);
    let second = store.display_live_tail(80, &options());

    assert!(first.len() > 1);
    assert_eq!(first.len(), second.len());
    assert_ne!(first, second);
    let plain = second
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    // 区块前空行与定稿 Reasoning 对齐
    assert!(plain[0].is_empty());
    assert!(plain[1].starts_with("◦ Thinking"));
    assert!(plain[1].contains("tokens"));
    assert!(plain.iter().any(|line| line.contains("inspect resize")));
    assert!(plain.iter().any(|line| line.contains("compare layout")));
    assert!(plain
        .iter()
        .all(|line| UnicodeWidthStr::width(line.as_str()) <= 80));
}

/// 【终端】【思考流式测试】验证实时与定稿思考共用折叠状态和正文布局。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn live_reasoning_preserves_fold_state_when_finalized() {
    let mut store = TranscriptStore::new(100);
    let source = (1..=12)
        .map(|index| format!("reasoning line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, &source));
    let options = super::TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    };

    let folded = store
        .display_live_tail(80, &options)
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert!(folded.iter().any(|line| line.contains("reasoning line 1")));
    assert!(folded.iter().any(|line| line.contains("reasoning line 12")));
    assert!(!folded.iter().any(|line| line.contains("reasoning line 6")));

    assert!(store.toggle_live_reasoning());
    let expanded_live = store
        .display_live_tail(80, &options)
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert!(expanded_live
        .iter()
        .any(|line| line.contains("reasoning line 6")));

    assert!(store.finalize_live_tail());
    let finalized = store
        .display_tail(80, &options)
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert!(finalized
        .iter()
        .any(|line| line.contains("reasoning line 6")));
}

/// 【终端】【工作状态测试】验证状态替换不会写入历史区。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn work_status_is_replaced_without_becoming_history() {
    let mut store = TranscriptStore::new(100);

    assert!(store.set_work_status(WorkStatus::WaitingResponse));
    let waiting = store.display_live_tail(80, &options());
    assert!(!waiting.is_empty());
    assert!(strip_ansi_for_test(waiting[0].as_str()).contains('s'));

    assert!(store.set_work_status(WorkStatus::Thinking));
    let thinking = store.display_live_tail(80, &options());
    assert!(!thinking.is_empty());
    let thinking_plain = strip_ansi_for_test(thinking[0].as_str());
    assert!(!thinking_plain.contains(&WorkStatus::WaitingResponse.localized_label()));
    assert!(thinking_plain.contains(&WorkStatus::Thinking.localized_label()));

    assert!(store.advance_live_animation());
    let animated = store.display_live_tail(80, &options());
    assert!(!animated.is_empty());
    assert!(
        strip_ansi_for_test(animated[0].as_str()).contains(&WorkStatus::Thinking.localized_label())
    );

    assert!(store.clear_work_status());
    assert!(store.display_live_tail(80, &options()).is_empty());
}

/// 【终端】【工作状态测试】验证工具结果之后继续显示 Working。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn tool_result_keeps_work_status_alive() {
    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Working));
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );
    store.push_tool_result("read_file".to_string(), true, "contents".to_string());

    let live = store.display_live_tail(80, &options());
    assert!(!live.is_empty(), "工具结果后工作动效行不应消失");
    assert!(live.iter().any(|line| {
        strip_ansi_for_test(line.as_str()).contains(&WorkStatus::Working.localized_label())
    }));
    assert!(store.advance_live_animation());
}

/// 【终端】【工作状态测试】验证实时思考会隐藏重复工作状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn work_status_hidden_when_live_reasoning_exists() {
    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Thinking));
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "inspect plan"));

    let live = store.display_live_tail(80, &options());
    let joined = strip_ansi_for_test(&live.iter().map(|line| line.as_str()).collect::<String>());
    assert!(!joined.contains(&WorkStatus::Working.localized_label()));
    assert!(joined.contains("Thinking") || joined.contains("思考"));
}

/// 【终端】【思考时态】思考定稿后标题改过去式，状态行不再闪 Thinking。
#[test]
fn finalized_reasoning_drops_thinking_work_status() {
    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Thinking));
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "inspect plan"));
    assert!(store.finalize_live_tail());

    let tail = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    let joined = tail.join("\n");
    assert!(joined.contains("Thought"), "{tail:?}");
    assert!(
        !joined.contains("Thinking"),
        "Thinking must not remain after finalize: {tail:?}"
    );
    let live = store.display_live_tail(80, &options());
    let live_plain: Vec<String> = live
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect();
    assert!(
        live_plain
            .iter()
            .any(|line| line.contains(&WorkStatus::WaitingResponse.localized_label())),
        "after thinking, status should wait for response: {live_plain:?}"
    );
}

/// 【终端】【子智能体动效】验证主 agent 空闲时子智能体动画仍持续推进。
///
/// 主视图下有运行中的子智能体时也要推进帧：底部 agent 面板的
/// 流光与实时统计依赖它；进入子智能体视图后继续驱动 Working 扫光。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn subagent_view_animates_while_main_agent_is_idle() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "检查项目".to_string(),
        "explore".to_string(),
        3,
    );
    let mut store = TranscriptStore::new(100);
    // 空 transcript 且主 agent 空闲：没有任何动画诉求，帧保持冻结
    assert!(!store.advance_live_animation());
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"检查项目"}"#.to_string(),
    );
    store.push_tool_result(
        "subagent".to_string(),
        true,
        format!(
            r#"{{"subagent":{{"id":"{}","status":"running"}}}}"#,
            subagent.id
        ),
    );

    // 主 agent 空闲但有运行中的子智能体：主视图面板动效仍需推进
    assert!(!store.viewing_running_subagent());
    assert!(
        store.advance_live_animation(),
        "主视图下运行中的子智能体必须驱动面板动效帧"
    );

    assert!(store.enter_subagent_view(0), "子智能体视图应能进入");
    assert!(store.viewing_running_subagent());
    assert!(
        store.advance_live_animation(),
        "运行中的子智能体视图必须持续推进动画帧"
    );
}
