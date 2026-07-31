use super::QueuedSubmission;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::transcript::TodoSnapshotItem;

/// 沉底面板展示的排队消息上限。
const QUEUE_PREVIEW_LIMIT: usize = 3;
/// 沉底面板展示的 todo 条目上限（进行中 + 后续待办）。
const TODO_PREVIEW_LIMIT: usize = 3;

/// 组装 composer 上方的沉底面板行（todo 快照 + 排队消息 + agent 面板）。
///
/// 参数:
/// - `todos`: 最新 todo 清单快照
/// - `queued`: 排队等待下一轮执行的用户提交
/// - `agent_lines`: 多智能体切换面板行（未截断）
/// - `cols`: 终端列数
///
/// 返回:
/// - 已截断到终端宽度的 ANSI 面板行；无内容时为空
pub(super) fn render_panel_lines(
    todos: &[TodoSnapshotItem],
    queued: &[QueuedSubmission],
    agent_lines: &[String],
    cols: usize,
) -> Vec<String> {
    let cols = cols.max(8);
    let mut lines = Vec::new();
    render_todo_section(todos, cols, &mut lines);
    render_queue_section(queued, cols, &mut lines);
    for line in agent_lines {
        lines.push(clip_line(line, cols));
    }
    lines
}

/// 渲染 todo 快照区：一行摘要 + 进行中与临近待办条目。
///
/// 参数:
/// - `todos`: 最新 todo 清单快照
/// - `cols`: 终端列数
/// - `lines`: 输出行缓冲
///
/// 返回:
/// - 无
fn render_todo_section(todos: &[TodoSnapshotItem], cols: usize, lines: &mut Vec<String>) {
    if todos.is_empty() {
        return;
    }
    let done = todos
        .iter()
        .filter(|item| item.status == "completed")
        .count();
    let cancelled = todos
        .iter()
        .filter(|item| item.status == "cancelled")
        .count();
    // 1. 全部完成/取消后不再占用底部空间
    if done + cancelled == todos.len() {
        return;
    }
    lines.push(clip_line(
        &format!(
            "\x1b[2m• {} {}/{}\x1b[0m",
            t("todo", "任务清单"),
            done,
            todos.len()
        ),
        cols,
    ));
    // 2. 只展示进行中与其后的待办，保持面板紧凑
    let mut shown = 0usize;
    for item in todos {
        if shown >= TODO_PREVIEW_LIMIT {
            break;
        }
        let rendered = match item.status.as_str() {
            "in_progress" => format!("\x1b[1m\x1b[36m  › {}\x1b[0m", item.text),
            "pending" => format!("\x1b[2m  ○ {}\x1b[0m", item.text),
            _ => continue,
        };
        lines.push(clip_line(&rendered, cols));
        shown += 1;
    }
}

/// 渲染排队消息区：一行计数 + 每条消息单行预览。
///
/// 参数:
/// - `queued`: 排队提交
/// - `cols`: 终端列数
/// - `lines`: 输出行缓冲
///
/// 返回:
/// - 无
fn render_queue_section(queued: &[QueuedSubmission], cols: usize, lines: &mut Vec<String>) {
    if queued.is_empty() {
        return;
    }
    lines.push(clip_line(
        &format!(
            "\x1b[2m• {} ({})\x1b[0m",
            t("queued for next turn", "已排队待下一轮"),
            queued.len()
        ),
        cols,
    ));
    for submission in queued.iter().take(QUEUE_PREVIEW_LIMIT) {
        let preview = submission
            .text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(clip_line(
            &format!("\x1b[2m\x1b[3m  ↳ {preview}\x1b[0m"),
            cols,
        ));
    }
    let hidden = queued.len().saturating_sub(QUEUE_PREVIEW_LIMIT);
    if hidden > 0 {
        lines.push(clip_line(
            &format!("\x1b[2m    … +{hidden} {}\x1b[0m", t("more", "条")),
            cols,
        ));
    }
}

/// 将单行 ANSI 文本截断到指定显示宽度（保留样式并补终止序列）。
///
/// 参数:
/// - `line`: 原始 ANSI 行
/// - `cols`: 终端列数
///
/// 返回:
/// - 显示宽度不超过 cols 的行
fn clip_line(line: &str, cols: usize) -> String {
    if visible_width(line) <= cols {
        return line.to_string();
    }
    let mut out = String::new();
    let mut width = 0usize;
    let mut chars = line.chars().peekable();
    // 1. 逐字符累计显示宽度，ANSI 序列不计宽；留 1 列给省略号
    let budget = cols.saturating_sub(1);
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            out.push(ch);
            if chars.peek() == Some(&'[') {
                for next in chars.by_ref() {
                    out.push(next);
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > budget {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push('…');
    out.push_str("\x1b[0m");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::cli::repl_clipboard::ReplClipboardState;

    /// 构造测试用排队提交。
    fn queued(text: &str) -> QueuedSubmission {
        QueuedSubmission {
            mode: AgentMode::Yolo,
            text: text.to_string(),
            clipboard: ReplClipboardState::default(),
        }
    }

    #[test]
    fn empty_inputs_produce_no_panel() {
        assert!(render_panel_lines(&[], &[], &[], 80).is_empty());
    }

    #[test]
    fn todo_section_shows_progress_and_active_items() {
        let todos = vec![
            TodoSnapshotItem {
                status: "completed".to_string(),
                text: "done one".to_string(),
            },
            TodoSnapshotItem {
                status: "in_progress".to_string(),
                text: "current".to_string(),
            },
            TodoSnapshotItem {
                status: "pending".to_string(),
                text: "next".to_string(),
            },
        ];
        let lines = render_panel_lines(&todos, &[], &[], 80);
        let joined = lines.join("\n");
        assert!(joined.contains("1/3"));
        assert!(joined.contains("current"));
        assert!(joined.contains("next"));
        assert!(!joined.contains("done one"));
    }

    #[test]
    fn fully_completed_todo_hides_panel() {
        let todos = vec![TodoSnapshotItem {
            status: "completed".to_string(),
            text: "done".to_string(),
        }];
        assert!(render_panel_lines(&todos, &[], &[], 80).is_empty());
    }

    #[test]
    fn queue_section_lists_previews_and_overflow() {
        let queue = vec![
            queued("one"),
            queued("two"),
            queued("three"),
            queued("four"),
        ];
        let lines = render_panel_lines(&[], &queue, &[], 80);
        let joined = lines.join("\n");
        assert!(joined.contains("(4)"));
        assert!(joined.contains("↳ one"));
        assert!(joined.contains("↳ three"));
        assert!(!joined.contains("↳ four"));
        assert!(joined.contains("+1"));
    }

    #[test]
    fn long_lines_are_clipped_to_terminal_width() {
        let queue = vec![queued(&"字".repeat(120))];
        let lines = render_panel_lines(&[], &queue, &[], 40);
        for line in &lines {
            assert!(visible_width(line) <= 40, "line too wide: {line:?}");
        }
        assert!(lines.iter().any(|line| line.contains('…')));
    }
}
