use crate::render::tool_view::{render, ToolView};
use crate::render::{activity_animation::strip_ansi_for_test, ToolCallDisplayMode};
use serde_json::json;

/// 【子任务卡片】【完成回归】读取真实统计并展示树形结论，历史没有统计时不补造数值。
/// 参数：无；返回：无，标题、统计或结果格式不符合要求时断言失败。
#[test]
fn completed_card_shows_subagent_metrics_and_tree_result() {
    let mut view = ToolView::running(
        "subagent".into(),
        json!({"action":"wait","subagent_id":"INTERNAL_ID"}).to_string(),
    );
    view.finish(true, json!({"subagent":{
        "id":"INTERNAL_ID", "description":"评估分布式缓存架构设计", "status":"completed",
        "step":4, "started_at":100, "updated_at":105,
        "stats":{"tool_calls":4,"total_tokens":1200,"token_estimate":1200,"token_estimate_is_actual":true},
        "result":"架构评审完毕：推荐采用 Redis 哨兵集群架构。"
    }}).to_string());
    for mode in [ToolCallDisplayMode::Summary, ToolCallDisplayMode::Full] {
        let text = strip_ansi_for_test(&render(&view, mode));
        let title = text.lines().next().unwrap();
        assert!(title.contains("Subagent 评估分布式缓存架构设计"), "{text}");
        assert!(
            title.contains("4 tool calls") && title.contains("1.2k tokens") && title.contains("5s"),
            "{text}"
        );
        assert!(
            !text.contains("Awaited")
                && !text.contains("completed")
                && !text.contains("INTERNAL_ID"),
            "{text}"
        );
        if mode == ToolCallDisplayMode::Full {
            assert!(text.contains("└ 架构评审完毕"), "{text}");
        }
    }
}

/// 【子任务卡片】【历史回归】缺失统计的旧记录只展示任务和结论，不能显示虚构用量或查询耗时。
/// 参数：无；返回：无，出现无来源的统计时断言失败。
#[test]
fn missing_statistics_do_not_invent_counts_or_duration() {
    let mut view = ToolView::running("subagent".into(), String::new());
    view.start_timer();
    view.finish(
        true,
        json!({"subagent":{"description":"历史任务","status":"completed","result":"已完成评审"}})
            .to_string(),
    );
    let text = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(
        text.contains("Subagent 历史任务") && text.contains("└ 已完成评审"),
        "{text}"
    );
    assert!(
        !text.contains("tokens") && !text.contains("tool calls") && !text.contains("ms"),
        "{text}"
    );
}

/// 【子任务卡片】【估算与异常回归】估算用量明确标记，失败原因与运行状态仍保持可见。
/// 参数：无；返回：无，估算冒充实际用量或异常被隐藏时断言失败。
#[test]
fn estimated_usage_and_failure_status_remain_explicit() {
    let mut view = ToolView::running("subagent".into(), json!({"action":"result"}).to_string());
    view.finish(false, json!({"subagent":{
        "description":"评审任务","status":"failed","error":"连接失败",
        "stats":{"tool_calls":0,"total_tokens":400,"token_estimate":1600,"token_estimate_is_actual":false}
    }}).to_string());
    let text = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Summary));
    assert!(
        text.contains("failed") && text.contains("连接失败"),
        "{text}"
    );
    assert!(
        text.contains("~1.6k tokens") && text.contains("0 tool calls"),
        "{text}"
    );
}
