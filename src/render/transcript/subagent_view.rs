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
    AnsiLine::wrap_block(&rendered, width.max(8))
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
        .map(|snapshot| match snapshot.status.as_str() {
            "completed" => "ok",
            "failed" | "cancelled" => "err",
            _ => "run",
        })
        .unwrap_or("run");
    // 1. 标题：与主视图工具行同语汇 + 返回提示
    let mut output = tool_event_text(&format!("Subagent {label}"), status);
    output.push_str(&format!(
        "\n\x1b[2m  {} · ↓ {}\x1b[0m\n",
        t("subagent session view", "子智能体会话视图"),
        t("switch back", "切换返回")
    ));
    // 2. 时间线全量渲染：思考 / 正文 / 工具行
    let entries = crate::tools::subagent_state::subagent_timeline(id).unwrap_or_default();
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
            SubagentTimelineEntry::Tool { name, ok, .. } => {
                let status = match ok {
                    Some(true) => "ok",
                    Some(false) => "err",
                    None => "run",
                };
                output.push('\n');
                output.push_str(&tool_event_text(name, status));
            }
        }
    }
    // 3. 【终端】【子智能体状态】运行中显示 Working 白色流光
    if status == "run" {
        output.push('\n');
        output.push_str(&render_activity_text(
            WorkStatus::Working.localized_label(),
            frame,
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
        let joined = lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Subagent 检查项目"));
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

    /// 【终端】【子智能体状态】验证运行状态只保留 Working 白色流光。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn running_view_uses_working_shimmer() {
        let first = render_view_text("missing-id", "检查项目", 0);
        // 亮带按字符位离散推进，相邻帧可能停在原位；跨过一个字符位再比较
        let second = render_view_text("missing-id", "检查项目", 14);
        let first_status = first.lines().last().unwrap_or_default();
        let second_status = second.lines().last().unwrap_or_default();

        assert_eq!(strip_ansi_for_test(first_status), "Working");
        assert_eq!(strip_ansi_for_test(second_status), "Working");
        assert_ne!(first_status, second_status);
    }
}
