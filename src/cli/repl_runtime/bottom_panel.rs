use super::QueuedSubmission;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::todo_style::{colorize_item, display_order, display_window, status_marker};
use crate::render::transcript::TodoSnapshotItem;

/// 沉底面板展示的排队消息上限。
const QUEUE_PREVIEW_LIMIT: usize = 3;

/// todo 多行模式展示的条目上限。
///
/// 全量铺开会让 composer 高度随清单长度实时变化，而 composer 贴在历史区
/// 下方——高度一变输入框就整体位移，模型工作时表现为输入框上下跳动。
const TODO_PREVIEW_LIMIT: usize = 5;

/// 组装 composer 上方的沉底面板行（todo 快照 + 排队消息 + agent 面板）。
///
/// 参数:
/// - `todos`: 最新 todo 清单快照
/// - `queued`: 排队等待下一轮执行的用户提交
/// - `agent_lines`: 多智能体切换面板行（未截断）
/// - `cols`: 终端列数
/// - `todo_compact`: true 时 todo 区只保留一行摘要（Ctrl+T 切换）
///
/// 返回:
/// - 已截断到终端宽度的 ANSI 面板行；无内容时为空
pub(super) fn render_panel_lines(
    todos: &[TodoSnapshotItem],
    queued: &[QueuedSubmission],
    agent_lines: &[String],
    cols: usize,
    todo_compact: bool,
) -> Vec<String> {
    let cols = cols.max(8);
    let mut lines = Vec::new();
    render_todo_section(todos, cols, todo_compact, &mut lines);
    render_queue_section(queued, cols, &mut lines);
    for line in agent_lines {
        lines.push(clip_line(line, cols));
    }
    lines
}

/// 渲染 todo 快照区。
///
/// 多行：已完成置顶 + 进行中 + 待办；单行：仅进度摘要（可带当前项）。
/// 全部完成时始终单行。
///
/// 参数:
/// - `todos`: 最新 todo 清单快照
/// - `cols`: 终端列数
/// - `compact`: 是否强制单行
/// - `lines`: 输出行缓冲
///
/// 返回:
/// - 无
fn render_todo_section(
    todos: &[TodoSnapshotItem],
    cols: usize,
    compact: bool,
    lines: &mut Vec<String>,
) {
    if todos.is_empty() {
        return;
    }
    let done = todos
        .iter()
        .filter(|item| item.status == "completed")
        .count();
    let total = todos.len();
    let all_done = done == total;
    let active = todos.iter().find(|item| item.status == "in_progress");
    let title = t("Plan", "计划");
    // 进度用 x/x，不再画 █░ 条——窄面板里数字更清楚，也不抖
    let mut header = if all_done {
        format!("\x1b[32m✓\x1b[0m \x1b[2m{title}\x1b[0m \x1b[2m{done}/{total}\x1b[0m")
    } else {
        format!("\x1b[2m{title}\x1b[0m \x1b[2m{done}/{total}\x1b[0m")
    };
    if let Some(item) = active {
        header.push_str(&format!(
            "  \x1b[2m·\x1b[0m {} {}",
            status_marker("in_progress"),
            colorize_item("in_progress", &item.text)
        ));
    }
    // 单行模式提示：多行可 Ctrl+T 收起；单行可展开
    if !all_done {
        let hint = if compact {
            t("Ctrl+T expand", "Ctrl+T 展开")
        } else {
            t("Ctrl+T compact", "Ctrl+T 单行")
        };
        header.push_str(&format!("  \x1b[2m{hint}\x1b[0m"));
    }
    lines.push(clip_line(&header, cols));

    if compact || all_done {
        return;
    }

    let statuses: Vec<&str> = todos.iter().map(|item| item.status.as_str()).collect();
    let order = display_order(&statuses);
    let ordered_statuses: Vec<&str> = order.iter().map(|&i| statuses[i]).collect();
    let (start, end) = display_window(&ordered_statuses, TODO_PREVIEW_LIMIT);
    for &index in &order[start..end] {
        let item = &todos[index];
        // 展开时跳过当前进行中项，避免与 header 重复
        if item.status == "in_progress" {
            continue;
        }
        let line = format!(
            "  {} {}",
            status_marker(&item.status),
            colorize_item(&item.status, &item.text)
        );
        lines.push(clip_line(&line, cols));
    }
    let hidden = total.saturating_sub(end - start);
    if hidden > 0 {
        lines.push(clip_line(
            &format!("\x1b[2m  … +{hidden} {}\x1b[0m", t("more", "条")),
            cols,
        ));
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
    use crate::render::activity_animation::strip_ansi_for_test;

    fn queued(text: &str) -> QueuedSubmission {
        QueuedSubmission {
            mode: AgentMode::Yolo,
            text: text.to_string(),
            clipboard: ReplClipboardState::default(),
        }
    }

    fn render(todos: &[TodoSnapshotItem], compact: bool) -> Vec<String> {
        render_panel_lines(todos, &[], &[], 80, compact)
    }

    #[test]
    fn empty_inputs_produce_no_panel() {
        assert!(render_panel_lines(&[], &[], &[], 80, false).is_empty());
    }

    #[test]
    fn todo_section_puts_completed_above_active_without_tree() {
        let todos = vec![
            TodoSnapshotItem {
                status: "pending".to_string(),
                text: "next".to_string(),
            },
            TodoSnapshotItem {
                status: "in_progress".to_string(),
                text: "current".to_string(),
            },
            TodoSnapshotItem {
                status: "completed".to_string(),
                text: "done one".to_string(),
            },
        ];
        let lines = render(&todos, false);
        let plain = strip_ansi_for_test(&lines.join("\n"));
        // 跳过标题行里挂的当前项，只比较条目区顺序
        let body = plain.lines().skip(1).collect::<Vec<_>>().join("\n");
        // 已完成条目保留在条目区
        assert!(body.contains("done one"));
        // 展开时当前项只在 header 展示，body 不再重复
        assert!(body.find("current").is_none());
        assert!(plain.contains('▶'));
        assert!(!plain.contains('├') && !plain.contains('└'));
        assert!(
            lines.join("\n").contains("\x1b[9m"),
            "completed items should use strikethrough"
        );
    }

    #[test]
    fn compact_mode_is_single_line_while_multi_lists_items() {
        let todos = vec![
            TodoSnapshotItem {
                status: "completed".to_string(),
                text: "done".to_string(),
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
        let compact = render(&todos, true);
        let multi = render(&todos, false);
        assert_eq!(compact.len(), 1);
        assert!(multi.len() > 1);
        let compact_plain = strip_ansi_for_test(&compact[0]);
        assert!(compact_plain.contains("1/3"));
        assert!(!compact_plain.contains('█') && !compact_plain.contains('░'));
        assert!(!compact_plain.contains("next"));
        assert!(multi.join("\n").contains("next"));
    }

    #[test]
    fn fully_completed_todo_collapses_to_one_line() {
        let todos = vec![TodoSnapshotItem {
            status: "completed".to_string(),
            text: "done".to_string(),
        }];
        let lines = render(&todos, false);
        assert_eq!(lines.len(), 1);
        let plain = strip_ansi_for_test(&lines[0]);
        assert!(plain.contains("1/1"));
        assert!(plain.starts_with('✓'));
    }

    fn todos(count: usize, in_progress: usize) -> Vec<TodoSnapshotItem> {
        (0..count)
            .map(|index| TodoSnapshotItem {
                status: if index == in_progress {
                    "in_progress".to_string()
                } else if index < in_progress {
                    "completed".to_string()
                } else {
                    "pending".to_string()
                },
                text: format!("任务{index}"),
            })
            .collect()
    }

    #[test]
    fn panel_height_stays_bounded_as_the_todo_list_grows() {
        let short = render(&todos(3, 1), false).len();
        let long = render(&todos(30, 10), false).len();
        assert!(
            long <= short + 4,
            "清单从 3 条增到 30 条，面板从 {short} 行涨到 {long} 行"
        );
    }

    #[test]
    fn todo_window_keeps_the_active_item_visible() {
        for focus in [0usize, 7, 19] {
            let rendered = render(&todos(20, focus), false).join("\n");
            let plain = strip_ansi_for_test(&rendered);
            assert!(
                plain.contains(&format!("任务{focus}")),
                "进行中项 {focus} 未出现在窗口内: {plain}"
            );
        }
    }

    #[test]
    fn hidden_todo_items_are_counted() {
        let rendered = render(&todos(12, 0), false).join("\n");
        let plain = strip_ansi_for_test(&rendered);
        assert!(plain.contains("0/12"));
        assert!(plain.contains('+'));
    }

    #[test]
    fn queue_section_lists_previews_and_overflow() {
        let queue = vec![
            queued("one"),
            queued("two"),
            queued("three"),
            queued("four"),
        ];
        let lines = render_panel_lines(&[], &queue, &[], 80, false);
        let joined = lines.join("\n");
        assert!(joined.contains("(4)"));
        assert!(joined.contains("↳ one"));
        assert!(joined.contains("+1"));
    }

    #[test]
    fn long_lines_are_clipped_to_terminal_width() {
        let queue = vec![queued(&"字".repeat(120))];
        let lines = render_panel_lines(&[], &queue, &[], 40, false);
        for line in &lines {
            assert!(visible_width(line) <= 40, "line too wide: {line:?}");
        }
        assert!(lines.iter().any(|line| line.contains('…')));
    }
}
