//! 工具卡片优化回归，覆盖默认预览、摘要与完整展开。

use super::{render, ToolView};
use crate::render::{
    activity_animation::strip_ansi_for_test as plain, render_expand, ToolCallDisplayMode,
};
use serde_json::{json, Value};

/// 【终端】【卡片样本】构造已结束的工具调用，不执行真实工具。
/// 参数: 工具名、参数和结果；返回: 完成视图。
fn completed(name: &str, args: Value, result: Value) -> ToolView {
    let mut view = ToolView::running(name.into(), args.to_string());
    view.finish(true, result.to_string());
    view
}

/// 【终端】【审批回归】通用和专用卡片均展示操作对象，不泄露敏感正文字段。
/// 参数: 无；返回: 无。
#[test]
fn approval_keeps_target_before_controls_for_all_views() {
    for name in [
        "write_file",
        "browser",
        "computer_use",
        "background_command",
        "run_command",
    ] {
        let mut view = ToolView::running(
            name.into(),
            json!({
                "action":"start", "path":"/tmp/target.txt", "command":"cargo check",
                "app":"Fixture", "password":"DO_NOT_PRINT", "text":"PRIVATE_CONTENT"
            })
            .to_string(),
        );
        view.request_permission("approval".into());
        let text = plain(&render(&view, ToolCallDisplayMode::Summary));
        assert!(text.contains("/tmp/target.txt"), "{name}: {text}");
        assert!(
            !text.contains("DO_NOT_PRINT") && !text.contains("PRIVATE_CONTENT"),
            "{text}"
        );
        assert!(text.contains("Action:") || text.contains("操作:"), "{text}");
        assert!(
            text.find("/tmp/target.txt").unwrap() < text.find("Allow once").unwrap(),
            "{text}"
        );
        assert_eq!(
            text.matches("Review scope:").count() + text.matches("当前审阅对象").count(),
            1
        );
        assert!(render(&view, ToolCallDisplayMode::Hidden).is_empty());
    }
}

/// 【终端】【后台回归】查询成功和任务失败分别呈现，标识完整内容保留在展开视图。
/// 参数: 无；返回: 无。
#[test]
fn background_list_counts_running_and_failed_without_done_badge() {
    let view = completed(
        "background_command",
        json!({"action":"list"}),
        json!({"ok":true,"tasks":[
            {"id":"abcdefgh-one-long-task", "status":"running", "command":"serve"},
            {"id":"abcdefgh-two-long-task", "status":"exited", "exit_code":2, "command":"build"}
        ]}),
    );
    let text = plain(&render_expand::with_force_collapse(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(
        text.contains("1 running") && text.contains("1 failed"),
        "{text}"
    );
    assert!(!text.contains("done"), "{text}");
    assert!(!text.contains("abcdefgh-one-long-task"), "{text}");
    assert!(
        text.contains("abcdefgh-o…") && text.contains("abcdefgh-t…"),
        "{text}"
    );
    assert!(text.contains("/ps to inspect logs or stop tasks"), "{text}");
    let full = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(full.contains("abcdefgh-one-long-task") && full.contains("abcdefgh-two-long-task"));
}

/// 【终端】【搜索分组回归】文件名只出现一次，正文保留冒号并按真实正则高亮。
/// 参数: 无；返回: 无。
#[test]
fn grep_groups_paths_and_highlights_regex_matches() {
    let view = completed(
        "grep",
        json!({"pattern":"中文|token"}),
        json!({"success":true,"stdout":
        "src/example.rs:9:中文: token\nsrc/example.rs:10:more token\nother.rs:1:token"}),
    );
    let ansi = render_expand::with_force_expand(|| render(&view, ToolCallDisplayMode::Full));
    let text = plain(&ansi);
    assert_eq!(text.matches("src/example.rs").count(), 1, "{text}");
    assert!(
        text.contains("9: 中文: token") && text.contains("10: more token"),
        "{text}"
    );
    assert!(ansi.contains("\x1b[1;33m中文\x1b[0m"), "{ansi:?}");
}

/// 【终端】【网页链接回归】预览优先显示标题与域名，全文保留安全地址供复制。
/// 参数: 无；返回: 无。
#[test]
fn web_preview_hides_long_url_and_details_keep_it() {
    let mut view = ToolView::running("web_search".into(), json!({"query":"docs"}).to_string());
    view.finish(true, "**Provider**: Local\n### 1. Documentation\n**URL**: https://example.com/docs?tracking=long-value\n**Snippet**: Read the guide.\n### 2. Unsafe\n**URL**: javascript:alert(1)\n**Snippet**: ignored link".into());
    let preview = plain(&render_expand::with_force_collapse(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(preview.contains("Documentation") && preview.contains("example.com"));
    assert!(!preview.contains("tracking="));
    let full = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(full.contains("https://example.com/docs?tracking=long-value"));
    assert!(!full.contains("javascript:"));
}

/// 【终端】【失败诊断回归】命令协议成功不掩盖非零退出，标准输出诊断不误报无错误输出。
/// 参数: 无；返回: 无。
#[test]
fn command_failure_keeps_exit_and_stdout_diagnostic() {
    let view = completed(
        "run_command",
        json!({"command":"build"}),
        json!({
            "success":false,"exit_code":17,"stdout":"fatal: dependency missing", "stderr":""
        }),
    );
    let text = plain(&render(&view, ToolCallDisplayMode::Summary));
    assert!(text.lines().next().unwrap().contains("17"), "{text}");
    assert!(text.contains("fatal: dependency missing"));
    assert!(!text.contains("without stderr"));
    let ansi = render(&view, ToolCallDisplayMode::Full);
    assert!(
        ansi.contains("\x1b[0mfatal: dependency missing"),
        "{ansi:?}"
    );
}

/// 【终端】【待办回归】完整计划保持原序，正文不在变更标题和列表重复，取消项单独计数。
/// 参数: 无；返回: 无。
#[test]
fn todo_preserves_plan_order_without_duplicate_change_text() {
    let view = completed(
        "todo",
        json!({"action":"update","status":"completed"}),
        json!({
        "changed":[{"text":"first step","status":"completed"}],"items":[
            {"text":"first step","status":"completed"},
            {"text":"second step","status":"in_progress"},
            {"text":"cancelled step","status":"cancelled"}
        ]}),
    );
    let text = plain(&render(&view, ToolCallDisplayMode::Full));
    assert_eq!(text.matches("first step").count(), 1, "{text}");
    assert!(text.find("first step").unwrap() < text.find("second step").unwrap());
    assert!(
        text.contains("1/2") && text.contains("1 cancelled"),
        "{text}"
    );
    assert!(text.contains("changed"), "{text}");
}

/// 【终端】【交互回归】标题优先可读名称，不把输入确认视为效果验证，摘要保留失败原因。
/// 参数: 无；返回: 无。
#[test]
fn interactions_separate_sent_input_and_effect_verification() {
    let mut view = completed(
        "computer_click",
        json!({"app":"42","window_id":101,"element_index":"g1:0"}),
        json!({"status":"clicked"}),
    );
    view.finish(true, "{\"status\":\"clicked\"}\nTarget: button Apply\nCall get_app_state for the updated state before choosing another action.".into());
    let summary = plain(&render(&view, ToolCallDisplayMode::Summary));
    assert!(summary.contains("Sent click · button Apply"), "{summary}");
    assert!(!summary.contains("101"));
    let full = plain(&render(&view, ToolCallDisplayMode::Full));
    assert!(full.contains("effect not verified") && full.contains("Window id: 101"));
    assert!(
        !full.contains("{\"") && !full.contains("Call get_app_state"),
        "{full}"
    );
    let mut browser = ToolView::running(
        "browser".into(),
        json!({"action":"click","ref":"e99"}).to_string(),
    );
    browser.finish(true, "Clicked button [ref=e99]\nPage: Preferences\nURL: https://example.com\nCall browser with action=snapshot to see the updated page.".into());
    let summary = plain(&render(&browser, ToolCallDisplayMode::Summary));
    assert!(
        summary.contains("Sent browser click · Preferences"),
        "{summary}"
    );
    assert!(!summary.contains("e99"));
    browser.finish(false, "stale reference".into());
    assert!(plain(&render(&browser, ToolCallDisplayMode::Summary)).contains("stale reference"));
}

/// 【终端】【图片回归】无论终端协议是否可用都保留数量和全部路径，摘要不读取图片。
/// 参数: 无；返回: 无。
#[test]
fn generated_images_keep_saved_paths_and_fold_additional_previews() {
    let view = completed(
        "generate_image",
        json!({}),
        json!({"type":"image_generation","images":[
            {"local_path":"/missing/first.png"}, {"local_path":"/missing/second.png"}
        ]}),
    );
    let summary = plain(&render(&view, ToolCallDisplayMode::Summary));
    assert!(summary.contains("2 images saved"), "{summary}");
    assert!(summary.contains("/missing/first.png") && summary.contains("/missing/second.png"));
    assert!(!summary.contains("preview unavailable"));
    let preview = plain(&render_expand::with_force_collapse(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert_eq!(
        preview.matches("Image preview unavailable.").count(),
        1,
        "{preview}"
    );
    assert!(preview.contains("1 more image previews") && preview.contains("Ctrl+O"));
    let full = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert_eq!(
        full.matches("Image preview unavailable.").count(),
        2,
        "{full}"
    );
}

/// 【终端】【归档回归】最后一项完成后活动清单清空，不把完成记录显示为删除。
/// 参数: 无；返回: 无。
#[test]
fn completed_todo_archive_is_not_reported_as_removed() {
    let view = completed(
        "todo",
        json!({"action":"update","status":"completed"}),
        json!({
            "changed":[{"text":"last task","status":"completed"}],"items":[]
        }),
    );
    let output = plain(&render(&view, ToolCallDisplayMode::Full));
    assert!(
        output.contains("last task") && output.contains('✓'),
        "{output}"
    );
    assert!(!output.contains("Removed"), "{output}");
}

/// 【终端】【正则回归】高亮使用原始模式，不能使用已替换制表符的标题文本。
/// 参数: 无；返回: 无。
#[test]
fn grep_highlight_uses_original_tab_pattern() {
    let view = completed(
        "grep",
        json!({"pattern":"a\tb"}),
        json!({"success":true,"stdout":"a.txt:1:a\tb"}),
    );
    let output = render(&view, ToolCallDisplayMode::Full);
    assert!(output.contains("\x1b[1;33ma"), "{output:?}");
}

/// 【终端】【同名待办回归】删除同名任务仍保留删除记录，未修改项不标记为变更。
/// 参数: 无；返回: 无。
#[test]
fn duplicate_todo_names_do_not_hide_removal_or_mark_other_items() {
    let view = completed(
        "todo",
        json!({"action":"remove","id":"first"}),
        json!({
            "changed":[{"id":"first","text":"same name","status":"pending"}],
            "items":[{"id":"second","text":"same name","status":"pending"}]
        }),
    );
    let output = plain(&render(&view, ToolCallDisplayMode::Full));
    assert!(output.contains("Removed:"), "{output}");
    assert!(!output.contains(" · changed"), "{output}");
}
