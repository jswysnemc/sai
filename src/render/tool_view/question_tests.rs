use super::{render, ToolView};
use crate::render::{activity_animation::strip_ansi_for_test, ToolCallDisplayMode};

/// 【提问展示】【测试数据】无参数，返回包含两个可选项的真实提问工具视图。
fn question_view() -> ToolView {
    ToolView::running("ask_question".into(), serde_json::json!({"questions":[{
        "header":"修改范围", "question":"这次修改哪些模块？", "multiple":false,
        "options":[{"label":"只改界面","description":"调整界面布局","value":"ui"},{"label":"全部模块","description":"实现与测试"}]
    }]}).to_string())
}

/// 【提问展示】【完成回归】摘要与展开都展示问题和用户答案，不暴露工具 JSON；无参数和返回值。
#[test]
fn question_completed_card_shows_answers_without_raw_json() {
    let mut view = question_view();
    view.finish(
        true,
        serde_json::json!({"status":"answered","answers":[{
        "header":"修改范围","question":"这次修改哪些模块？","answer":"ui","image_count":2
    }],"instruction":"INTERNAL_INSTRUCTION"})
        .to_string(),
    );
    for mode in [ToolCallDisplayMode::Summary, ToolCallDisplayMode::Full] {
        let text = strip_ansi_for_test(&render(&view, mode));
        assert!(text.contains("Asked"));
        assert!(text.contains("这次修改哪些模块？"), "{text}");
        assert!(text.contains("只改界面"), "{text}");
        assert!(
            !text.contains("args:")
                && !text.contains("output:")
                && !text.contains("INTERNAL_INSTRUCTION"),
            "{text}"
        );
    }
}

/// 【提问展示】【异常回归】取消时显示明确原因，展开也不回退 JSON；无参数和返回值。
#[test]
fn question_cancelled_card_shows_reason_without_raw_json() {
    let mut view = question_view();
    view.finish(
        false,
        crate::question::unavailable_tool_output("user cancelled the question"),
    );
    let text = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(text.contains("cancelled"));
    assert!(
        !text.contains("args:") && !text.contains("output:"),
        "{text}"
    );
}

/// 【提问展示】【多选与恢复】仅有结果的历史仍展示全部答案和图片计数；无参数和返回值。
#[test]
fn question_history_renders_multiple_answers_without_arguments() {
    let mut view = ToolView::running("ask_question".into(), String::new());
    view.finish(true, serde_json::json!({"status":"answered","answers":[{"header":"范围","question":"修改哪些模块？","answer":["界面","数据\n补充说明"],"image_count":2}]}).to_string());
    let text = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(text.contains("修改哪些模块？") && text.contains("界面") && text.contains("补充说明"));
    assert!(text.contains("2 attached images"));
    assert!(!text.contains("output:"));
}
