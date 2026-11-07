use super::*;
use crate::render::{ansi_clip::strip_ansi, render_expand};
use serde_json::json;

/// 【终端】【目标夹具】构造已完成工具视图，不执行真实目标工具。
/// 参数: `name` 为工具名，`output` 为结构化结果；返回: 成功完成的工具视图。
fn view(name: &str, output: Value) -> ToolView {
    let mut view = ToolView::running(name.into(), "{}".into());
    view.finish(true, output.to_string());
    view
}

/// 【终端】【目标测试】通过共用入口展开目标卡片并移除渲染样式。
/// 参数: `view` 为工具视图，`mode` 为展示模式；返回: 可断言的纯文本。
fn displayed(view: &ToolView, mode: ToolCallDisplayMode) -> String {
    strip_ansi(&crate::i18n::with_locale(crate::i18n::Locale::Zh, || {
        render_expand::with_force_expand(|| super::super::render(view, mode))
    }))
}

/// 【终端】【动作回归】创建、查看和更新均使用中文动作，并兼容两种返回结构。
/// 参数: 无；返回: 无。
#[test]
fn actions_distinguish_create_get_and_update() {
    for (name, action) in [
        ("create_goal", "创建目标"),
        ("get_goal", "查看目标"),
        ("update_goal", "更新目标"),
    ] {
        let goal = json!({"objective":"完成验证", "status":"active"});
        let output = if name == "get_goal" {
            json!({"goal":goal})
        } else {
            goal
        };
        let text = displayed(&view(name, output), ToolCallDisplayMode::Full);
        let title = text.lines().next().unwrap();
        assert!(
            title.contains(&format!("{action} · 进行中 · 完成验证")),
            "{text}"
        );
        assert!(text.contains("目标：\n    完成验证"), "{text}");
        assert!(!text.contains("最近进度："), "{text}");
    }
}

/// 【终端】【多行目标回归】标题有界且单行，展开正文保留目标的全部段落并分离进度。
/// 参数: 无；返回: 无。
#[test]
fn full_mode_preserves_long_multiline_objective_and_progress() {
    let objective = (1..=20)
        .map(|i| format!("第{i}步：验证目标的完整内容"))
        .collect::<Vec<_>>()
        .join("\n");
    let goal = json!({"objective":objective, "status":"active", "updates":[
        {"kind":"progress", "message":"已完成第一轮验证\n下一步补齐边界测试"}
    ]});
    let text = displayed(
        &view("get_goal", json!({"goal":goal})),
        ToolCallDisplayMode::Full,
    );
    let title = text.lines().next().unwrap();
    assert!(title.contains('…'), "{title}");
    assert!(
        crate::render::ansi_clip::ansi_visible_width(title) <= 80,
        "{title}"
    );
    let expected = format!("目标：\n    {}", objective.replace('\n', "\n    "));
    assert!(text.contains(&expected), "{text}");
    assert!(
        text.contains("最近进度：\n    已完成第一轮验证\n    下一步补齐边界测试"),
        "{text}"
    );
}

/// 【终端】【状态回归】所有持久化状态使用中文标签和对应颜色语义，兼容完成别名及未知状态。
/// 参数: 无；返回: 无。
#[test]
fn statuses_are_localized_and_keep_health() {
    for (status, label, health) in [
        ("active", "进行中", ToolHealth::Neutral),
        ("paused", "已暂停", ToolHealth::Neutral),
        ("blocked", "受阻", ToolHealth::Err),
        ("usage_limited", "用量受限", ToolHealth::Err),
        ("budget_limited", "预算受限", ToolHealth::Err),
        ("complete", "已完成", ToolHealth::Ok),
        ("completed", "已完成", ToolHealth::Ok),
        ("future", "未知状态（future）", ToolHealth::Neutral),
    ] {
        assert_eq!(
            crate::i18n::with_locale(crate::i18n::Locale::Zh, || status_label(status)),
            (label.into(), health)
        );
        for mode in [ToolCallDisplayMode::Full, ToolCallDisplayMode::Summary] {
            let text = displayed(
                &view("update_goal", json!({"objective":"验证", "status":status})),
                mode,
            );
            assert!(text.lines().next().unwrap().contains(label), "{text}");
        }
    }
    let text = displayed(
        &view("get_goal", json!({"objective":"验证"})),
        ToolCallDisplayMode::Full,
    );
    assert!(text.contains("进行中"));
}

/// 【终端】【最新进度回归】忽略旧进度、空记录和系统更新，摘要保留最近进度且不复述完整目标。
/// 参数: 无；返回: 无。
#[test]
fn summary_shows_latest_nonempty_progress_not_history() {
    let goal = json!({
        "id":"INTERNAL_ID", "objective":format!("{}\n隐藏的目标尾段", "完整目标".repeat(30)),
        "status":"active", "created_at":"INTERNAL_TIME", "updates":[
            {"kind":"progress", "message":"OLD_PROGRESS"},
            {"kind":"progress", "message":"已通过核心测试\n正在验证边界"},
            {"kind":"status", "message":"STATUS_HISTORY"},
            {"kind":"account", "message":"ACCOUNT_HISTORY"},
            {"kind":"progress", "message":" \n\t"},
            {"kind":"progress", "message":null}
        ]
    });
    let goal_view = view("update_goal", goal);
    for mode in [ToolCallDisplayMode::Full, ToolCallDisplayMode::Summary] {
        let text = displayed(&goal_view, mode);
        assert!(text.contains("已通过核心测试"), "{text}");
        for hidden in [
            "OLD_PROGRESS",
            "STATUS_HISTORY",
            "ACCOUNT_HISTORY",
            "INTERNAL_ID",
            "INTERNAL_TIME",
        ] {
            assert!(!text.contains(hidden), "{text}");
        }
        if mode == ToolCallDisplayMode::Summary {
            assert!(
                text.contains("最近进度：已通过核心测试 正在验证边界"),
                "{text}"
            );
            assert!(!text.contains("隐藏的目标尾段"), "{text}");
            assert!(!text.contains("目标："), "{text}");
            assert_eq!(text.lines().count(), 2, "{text}");
        }
    }
}

/// 【终端】【进度边界回归】摘要截断长多行进度，缺失或无效历史不产生空分区。
/// 参数: 无；返回: 无。
#[test]
fn summary_bounds_progress_and_omits_empty_sections() {
    let text = displayed(
        &view(
            "get_goal",
            json!({"objective":"验证", "updates":[
                {"kind":"progress", "message":format!("{}\n不可见进度尾段", "多行进度".repeat(40))}
            ]}),
        ),
        ToolCallDisplayMode::Summary,
    );
    assert!(text.contains("最近进度：多行进度"));
    assert!(text.contains('…'));
    assert!(!text.contains("不可见进度尾段"));
    for updates in [
        Value::Null,
        json!([]),
        json!({}),
        json!([{"kind":"account", "message":"计费"}]),
    ] {
        let text = displayed(
            &view("get_goal", json!({"objective":"验证", "updates":updates})),
            ToolCallDisplayMode::Summary,
        );
        assert!(!text.contains("最近进度"));
        assert_eq!(text.lines().count(), 1);
    }
}

/// 【终端】【预算回归】两个模式均保留显式正预算，未设预算不展示内部用量。
/// 参数: 无；返回: 无。
#[test]
fn explicit_budget_is_retained_in_both_modes() {
    for mode in [ToolCallDisplayMode::Full, ToolCallDisplayMode::Summary] {
        for used in [0, 1200] {
            let text = displayed(
                &view(
                    "get_goal",
                    json!({"objective":"验证", "token_budget":1000, "tokens_used":used}),
                ),
                mode,
            );
            assert!(
                text.contains(&format!("预算：{used} / 1000 tokens")),
                "{text}"
            );
        }
        let text = displayed(
            &view("get_goal", json!({"objective":"验证", "token_budget":1000})),
            mode,
        );
        assert!(text.contains("预算：0 / 1000 tokens"));
        for budget in [Value::Null, json!(0), json!(-1), json!("1000")] {
            let text = displayed(
                &view(
                    "get_goal",
                    json!({"objective":"验证", "token_budget":budget, "tokens_used":98765}),
                ),
                mode,
            );
            assert!(
                !text.contains("预算：") && !text.contains("98765"),
                "{text}"
            );
        }
    }
}

/// 【终端】【回退回归】无目标保持既有卡片，错误、无效结果及运行中结果交回共用渲染。
/// 参数: 无；返回: 无。
#[test]
fn no_goal_and_errors_keep_existing_fallbacks() {
    for mode in [ToolCallDisplayMode::Full, ToolCallDisplayMode::Summary] {
        for output in [Value::Null, json!({"goal":null})] {
            let text = displayed(&view("get_goal", output), mode);
            assert!(text.contains("尚未设置目标"));
            assert_eq!(text.lines().count(), 1);
        }
        for output in [
            json!({}),
            json!({"objective":" \n"}),
            json!({"objective":17}),
        ] {
            assert!(render(&view("get_goal", output), mode).is_none());
        }
        let mut invalid = ToolView::running("get_goal".into(), "{}".into());
        assert!(render(&invalid, mode).is_none());
        invalid.finish(true, "not json".into());
        assert!(render(&invalid, mode).is_none());
        invalid.finish(false, "goal failed".into());
        assert!(render(&invalid, mode).is_none());
        assert!(displayed(&invalid, mode).contains("goal failed"));
        assert!(render(&view("other_tool", json!({"objective":"验证"})), mode).is_none());
    }
}

/// 【终端】【安全回归】外部目标、进度及未知状态不能注入终端控制序列，隐藏模式不泄露正文。
/// 参数: 无；返回: 无。
#[test]
fn full_summary_and_hidden_are_terminal_safe() {
    let goal_view = view(
        "update_goal",
        json!({
            "objective":"目标\u{001b}[2J\u{001b}[31m红字\u{001b}[0m\r尾部\u{0007}\n第二行",
            "status":"future\u{001b}[2J\n状态",
            "updates":[{"kind":"progress", "message":"进度\u{001b}]0;恶意标题\u{0007}安全\r\u{0008}\n下一行"}]
        }),
    );
    for mode in [ToolCallDisplayMode::Full, ToolCallDisplayMode::Summary] {
        let raw = render(&goal_view, mode).unwrap();
        assert!(!raw.contains("\u{001b}[2J") && !raw.contains("\u{001b}]0;"));
        let text = strip_ansi(&raw);
        assert!(text.contains("目标") && text.contains("进度"), "{text}");
        assert!(!text.contains("恶意标题"), "{text}");
        assert!(
            !text
                .chars()
                .any(|ch| ch.is_control() && ch != '\n' && ch != '\t'),
            "{text:?}"
        );
    }
    assert_eq!(
        render(&goal_view, ToolCallDisplayMode::Hidden),
        Some(String::new())
    );
    assert!(displayed(&goal_view, ToolCallDisplayMode::Hidden).is_empty());
}
