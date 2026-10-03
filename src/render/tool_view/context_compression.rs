use super::model::ToolView;
use crate::render::activity_animation::render_activity_line;
use crate::render::terminal_text as t;
use crate::render::tool_event_line::tool_event_text;
use serde_json::Value;

/// 【上下文】【压缩反馈】返回摘要生成或归档执行阶段的终端文案
/// 参数: preparing 表示模型正在生成摘要参数；返回状态标题
pub(crate) fn compression_progress_label(preparing: bool) -> &'static str {
    if preparing {
        t("Preparing context summary", "正在生成上下文摘要")
    } else {
        t("Compressing context", "正在压缩上下文")
    }
}

/// 【上下文】【压缩反馈】将工具回执转换为正文占用变化或失败原因
/// 参数: ok 表示执行成功，output 为工具回执；返回紧凑结果标题
pub(crate) fn compression_result_label(ok: bool, output: &str) -> String {
    if !ok {
        let reason: String = output
            .chars()
            .filter(|c| !c.is_control())
            .take(240)
            .collect();
        return format!(
            "{}: {reason}",
            t("Context compression failed", "上下文压缩失败")
        );
    }
    let receipt = serde_json::from_str::<Value>(output).ok();
    let counts = receipt.as_ref().and_then(|value| {
        Some((
            value.get("before_tokens")?.as_u64()?,
            value.get("after_tokens")?.as_u64()?,
        ))
    });
    match counts {
        Some((before, after)) => format!(
            "{} · {} {before} → {after} token",
            t("Context compressed", "上下文已压缩"),
            t("estimated result text", "结果正文估算")
        ),
        None => t("Context compressed", "上下文已压缩").to_string(),
    }
}

/// 【上下文】【压缩反馈】渲染实时及历史压缩调用，隐藏普通工具时仍保留反馈
/// 参数: view 为工具生命周期，frame 为动效帧；返回终端显示文本
pub(super) fn render(view: &ToolView, frame: usize) -> String {
    match &view.outcome {
        Some(outcome) => tool_event_text(
            &compression_result_label(outcome.ok, &outcome.output),
            if outcome.ok { "ok" } else { "err" },
        ),
        None => render_activity_line(
            compression_progress_label(serde_json::from_str::<Value>(&view.arguments).is_err()),
            "",
            frame,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ToolCallDisplayMode;

    /// 【上下文】【终端回归】隐藏工具详情时，压缩开始、成功及错误仍显示
    /// 参数: 无；返回无
    #[test]
    fn compression_feedback_survives_hidden_tool_mode() {
        let mut view = ToolView::running("compress_context".into(), "{}".into());
        let started = crate::render::activity_animation::strip_ansi_for_test(
            &super::super::render(&view, ToolCallDisplayMode::Hidden),
        );
        assert!(started.contains(compression_progress_label(false)));
        view.outcome = Some(super::super::model::ToolOutcome {
            ok: true,
            output: r#"{"before_tokens":12000,"after_tokens":800}"#.into(),
        });
        let rendered = super::super::render(&view, ToolCallDisplayMode::Hidden);
        assert!(rendered.contains("12000 → 800 token"));
        view.outcome = Some(super::super::model::ToolOutcome {
            ok: false,
            output: "stale context revision".into(),
        });
        let rendered = super::super::render(&view, ToolCallDisplayMode::Hidden);
        assert!(rendered.contains("stale context revision"));
        assert!(!rendered.contains("12000"));
    }
}
