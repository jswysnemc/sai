use super::line::AnsiLine;
use super::{markdown_cell, reasoning_cell};
use crate::i18n::text as t;
use crate::render::activity_animation::render_activity_text;
use crate::render::tool_event_line::tool_event_text;
use crate::render::work_status::WorkStatus;
use crate::tools::subagent_timeline::SubagentTimelineEntry;

/// 渲染子智能体会话视图（标题 + 完整时间线）。
///
/// 参数:
/// - `id`: 子智能体 ID
/// - `label`: 展示名称
/// - `width`: 当前终端列数
/// - `frame`: live 动画帧序号
///
/// 返回:
/// - 预换行 ANSI 行
pub(super) fn render_view_lines(
    id: &str,
    label: &str,
    width: usize,
    frame: usize,
) -> Vec<AnsiLine> {
    // 渲染宽度上下文与折行共用同一宽度，避免表格/思考块超宽被二次折断
    let rendered = crate::render::render_width::with_render_width(width, || {
        render_view_text(id, label, frame)
    });
    AnsiLine::wrap_block(&rendered, width.max(1))
}

/// 生成子智能体会话视图的 ANSI 文本。
///
/// 参数:
/// - `id`: 子智能体 ID
/// - `label`: 展示名称
/// - `frame`: live 动画帧序号
///
/// 返回:
/// - ANSI 文本块
fn render_view_text(id: &str, label: &str, frame: usize) -> String {
    let snapshot = crate::tools::subagent_state::subagent_snapshot(id).ok();
    let status = snapshot
        .as_ref()
        .map(|snapshot| crate::render::status_style::subagent_status_key(&snapshot.status))
        .unwrap_or("run");
    // 1. 标题：与主视图工具行同语汇（Delegating/Delegated）+ 返回提示
    let tense = crate::render::tool_event_line::ToolVerbTense::from_done(status != "run");
    let verb = crate::render::tool_event_line::tool_verb("subagent", tense);
    let mut output = tool_event_text(&format!("{verb} {label}"), status);
    output.push_str(&format!(
        "\n\x1b[2m  └ {} · ↓ {}\x1b[0m\n",
        t("subagent session view", "子智能体会话视图"),
        t("switch back", "切换返回")
    ));
    // 2. 时间线全量渲染：思考 / 正文 / 工具行
    let entries = crate::tools::subagent_state::subagent_timeline(id).unwrap_or_default();
    if let Some(error) = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.error.as_deref())
    {
        let (style, heading) = match status {
            "err" => ("\x1b[31m", t("Failure details", "失败详情")),
            "interrupted" => ("\x1b[2m", t("Interruption details", "中断详情")),
            _ => ("\x1b[2m", t("Task details", "任务详情")),
        };
        output.push_str(&format!(
            "\n{style}{heading}\x1b[0m\n{}\n",
            markdown_cell::render_completed(error)
        ));
    }
    if entries.is_empty() {
        output.push_str(&format!(
            "\n\x1b[2m  {}\x1b[0m",
            t("no timeline yet", "暂无时间线")
        ));
    }
    for entry in &entries {
        match entry {
            SubagentTimelineEntry::Reasoning { text } => {
                output.push('\n');
                output.push_str(&reasoning_cell::render_thinking_body(
                    text, false, false, None,
                ));
            }
            SubagentTimelineEntry::Text { text } => {
                output.push('\n');
                output.push_str(&markdown_cell::render_completed(text));
            }
            SubagentTimelineEntry::Message { from, text } => {
                // 追加消息与子智能体自身输出明确区分：来源行着色，正文弱化缩进
                let source = if from == "user" {
                    t("user message", "用户留言")
                } else {
                    t("message from parent agent", "主代理消息")
                };
                output.push('\n');
                output.push_str(&format!("\x1b[38;5;39m● {source}\x1b[0m"));
                for (index, line) in text.lines().enumerate() {
                    let gutter = if index == 0 { "└ " } else { "  " };
                    output.push_str(&format!("\n\x1b[2m  {gutter}{line}\x1b[0m"));
                }
            }
            SubagentTimelineEntry::Tool {
                name,
                args_preview,
                ok,
                output_preview,
                ..
            } => {
                let status = match ok {
                    Some(true) => "ok",
                    Some(false) => "err",
                    None if matches!(status, "interrupted" | "cancelled" | "err") => status,
                    None => "run",
                };
                // 与主视图工具行同语汇：动词 + 对象，而不是原始工具名
                let tense =
                    crate::render::tool_event_line::ToolVerbTense::from_done(status != "run");
                let label = crate::render::tool_event_line::tool_event_label_tense(
                    name,
                    Some(args_preview.as_str()),
                    tense,
                );
                output.push('\n');
                let heading = tool_event_text(&label, status);
                if status == "run" {
                    output.push_str(&crate::render::content_indent::animate_guide_marker(
                        &heading, frame,
                    ));
                } else {
                    output.push_str(&heading);
                }
                if let Some(result) = output_preview
                    .as_deref()
                    .filter(|text| ok == &Some(false) && !text.trim().is_empty())
                {
                    let body = crate::render::command_result_block::render_live_command_output(
                        "", result, false,
                    );
                    output.push('\n');
                    output.push_str(&body);
                }
            }
        }
    }
    if !entries.iter().any(
        |entry| matches!(entry, SubagentTimelineEntry::Text { text } if !text.trim().is_empty()),
    ) {
        if let Some(result) = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.result.as_deref())
            .filter(|text| !text.trim().is_empty())
        {
            output.push('\n');
            output.push_str(&markdown_cell::render_completed(result));
        }
    }
    // 3. 【终端】【子智能体状态】运行中显示 Working 主题流光；待命给出留言提示
    if status == "run" {
        output.push('\n');
        let label = WorkStatus::Working.localized_label();
        output.push_str(&render_activity_text(&label, frame));
    } else if status == "idle" {
        output.push('\n');
        output.push_str(&format!(
            "\x1b[38;5;110m● {}\x1b[0m \x1b[2m{}\x1b[0m",
            t("idle, waiting for follow-ups", "待命中"),
            t("leave a message with /msg", "可用 /msg 留言追加指令")
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    #[test]
    fn view_renders_title_and_placeholder_without_snapshot() {
        let lines = render_view_lines("missing-id", "检查项目", 80, 0);
        let joined = strip_ansi_for_test(
            &lines
                .iter()
                .map(|line| line.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert!(joined.contains("Delegating 检查项目"));
        assert!(joined.contains("暂无时间线") || joined.contains("no timeline"));
    }

    #[test]
    fn view_lines_fit_width() {
        let lines = render_view_lines("missing-id", &"标".repeat(60), 40, 0);
        for line in &lines {
            let width: usize = line
                .as_str()
                .chars()
                .filter(|ch| !ch.is_control())
                .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
                .sum();
            let _ = width;
        }
        assert!(!lines.is_empty());
    }

    /// 【终端】【子智能体状态】验证运行状态只保留 Working 主题流光。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn running_view_uses_working_shimmer() {
        let first = render_view_text("missing-id", "检查项目", 0);
        // 【终端】【流光测试】对比扫描带在文字外与文字中的阶段，覆盖 ANSI 降级样式
        let second = render_view_text("missing-id", "检查项目", 31);
        let first_status = first.lines().last().unwrap_or_default();
        let second_status = second.lines().last().unwrap_or_default();

        assert_eq!(strip_ansi_for_test(first_status), "Working");
        assert_eq!(strip_ansi_for_test(second_status), "Working");
        assert_ne!(first_status, second_status);
    }

    /// 会话恢复后的中断任务保留中断原因，概要与详情都停止运行提示和动效。
    #[test]
    fn restored_interrupted_subagent_is_static_and_neutral() {
        let temp = tempfile::tempdir().unwrap();
        let owner = temp.path().to_string_lossy().into_owned();
        let id = format!("restored-view-{}", rand::random::<u64>());
        let record = serde_json::json!([{
            "owner_key": owner,
            "snapshot": {
                "id": id, "description": "检查项目", "subagent_type": "explore",
                "status": "running", "max_steps": 8, "step": 2,
                "started_at": 1, "updated_at": 2
            },
            "timeline": [{
                "kind": "tool", "step": 2, "name": "read_file",
                "args_preview": "{\"path\":\"src/main.rs\"}", "ok": null,
                "output_preview": null
            }, {"kind": "text", "text": "Recovered partial response"}],
            "finish_notified": true
        }]);
        std::fs::write(
            temp.path().join("subagents.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let snapshots = crate::tools::subagent_state::list_subagents_for_owner(&owner);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].status, "interrupted");

        let cell = super::super::subagent_cell::SubagentCell::new(
            serde_json::json!({"action": "status", "subagent_id": id}).to_string(),
        );
        let overview = cell.overview();
        assert_eq!(overview.status, "interrupted");
        assert!(!overview.running);
        assert!(!cell.has_live_updates());
        assert_eq!(overview.detail, snapshots[0].error);
        let first = render_view_text(&id, "检查项目", 4);
        let later = render_view_text(&id, "检查项目", 12);
        assert_eq!(first, later);
        let plain = strip_ansi_for_test(&first);
        assert!(plain.contains(t("Interruption details", "中断详情")));
        assert!(plain.contains("Recovered partial response"));
        assert!(
            plain.contains(snapshots[0].error.as_deref().unwrap()),
            "{plain}"
        );
        assert!(!plain.contains("Working"));
        assert!(!plain.contains(t("Failure details", "失败详情")));
        assert!(!first.contains("\x1b[31m"));
        crate::tools::subagent_state::clear_subagents_for_owner(&owner);
    }

    /// 未记录正文时间线时，已完成任务仍展示没有尾部换行的完整结果。
    #[test]
    fn completed_subagent_shows_unterminated_result() {
        let owner = format!("view-result-{}", rand::random::<u64>());
        let (snapshot, _cancel) = crate::tools::subagent_state::create_subagent_for_owner(
            &owner,
            "检查项目".into(),
            "explore".into(),
            8,
        );
        crate::tools::subagent_state::finish_subagent(
            &snapshot.id,
            "completed",
            Some("Result without trailing newline".into()),
            None,
            None,
        );
        let text = strip_ansi_for_test(&render_view_text(&snapshot.id, "检查项目", 4));
        assert!(text.contains("Result without trailing newline"), "{text}");
        assert!(!text.contains("Working"));
        crate::tools::subagent_state::clear_subagents_for_owner(&owner);
    }
}
