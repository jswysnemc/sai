use super::QueuedSubmission;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::transcript::TodoSnapshotItem;

/// 沉底面板展示的排队消息上限。
const QUEUE_PREVIEW_LIMIT: usize = 3;

/// todo 区展示的条目上限。
///
/// 全量铺开会让 composer 高度随清单长度实时变化，而 composer 贴在历史区
/// 下方——高度一变输入框就整体位移，模型工作时表现为输入框上下跳动。
/// 固定为一个围绕进行中项的窗口后，高度只在极少数边界变化。
const TODO_PREVIEW_LIMIT: usize = 5;

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

/// 渲染 todo 快照区：一行摘要 + 围绕进行中项的条目窗口。
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
    lines.push(clip_line(
        &format!(
            "\x1b[2m• {} {}/{}\x1b[0m",
            t("todo", "任务清单"),
            done,
            todos.len()
        ),
        cols,
    ));
    // 只展示围绕进行中项的固定窗口：全量铺开会让面板高度随清单长度变化，
    // 连带 composer 上下位移
    let window = todo_window(todos);
    for item in window {
        let rendered = match item.status.as_str() {
            "in_progress" => format!("\x1b[1m\x1b[36m  › {}\x1b[0m", item.text),
            "pending" => format!("\x1b[2m  ○ {}\x1b[0m", item.text),
            "completed" => format!("\x1b[2m\x1b[32m  ✓ {}\x1b[0m", item.text),
            "cancelled" => format!("\x1b[2m\x1b[9m  × {}\x1b[0m", item.text),
            _ => format!("\x1b[2m  ○ {}\x1b[0m", item.text),
        };
        lines.push(clip_line(&rendered, cols));
    }
    let hidden = todos.len().saturating_sub(window.len());
    if hidden > 0 {
        lines.push(clip_line(
            &format!("\x1b[2m    … +{hidden} {}\x1b[0m", t("more", "条")),
            cols,
        ));
    }
}

/// 选取围绕进行中项的 todo 展示窗口。
///
/// 条目多于上限时以第一条进行中项为中心取窗口，让当前工作始终可见；
/// 无进行中项时取开头。
///
/// 参数:
/// - `todos`: 完整清单
///
/// 返回:
/// - 窗口内条目
fn todo_window(todos: &[TodoSnapshotItem]) -> &[TodoSnapshotItem] {
    if todos.len() <= TODO_PREVIEW_LIMIT {
        return todos;
    }
    let focus = todos
        .iter()
        .position(|item| item.status == "in_progress")
        .unwrap_or(0);
    // 焦点尽量居中，同时保证窗口不越过清单两端
    let half = TODO_PREVIEW_LIMIT / 2;
    let start = focus
        .saturating_sub(half)
        .min(todos.len() - TODO_PREVIEW_LIMIT);
    &todos[start..start + TODO_PREVIEW_LIMIT]
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
        assert!(joined.contains("done one"));
    }

    /// 全部完成后仍展示计划，便于沉底核对。
    #[test]
    fn fully_completed_todo_remains_visible() {
        let todos = vec![TodoSnapshotItem {
            status: "completed".to_string(),
            text: "done".to_string(),
        }];
        let rendered = render_panel_lines(&todos, &[], &[], 80).join("\n");
        assert!(rendered.contains("1/1"));
        assert!(rendered.contains("done"));
    }

    /// 构造指定数量的待办条目。
    ///
    /// 参数:
    /// - `count`: 条目数量
    /// - `in_progress`: 进行中项的下标
    ///
    /// 返回:
    /// - 待办清单
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

    /// 【终端】【底部面板】验证面板高度不随清单长度线性增长。
    ///
    /// 面板行数直接决定 composer 高度，而 composer 贴在历史区下方：
    /// 高度一变输入框就整体位移，模型工作时表现为输入框上下跳动。
    #[test]
    fn panel_height_stays_bounded_as_the_todo_list_grows() {
        let short = render_panel_lines(&todos(3, 1), &[], &[], 80).len();
        let long = render_panel_lines(&todos(30, 10), &[], &[], 80).len();

        assert!(
            long <= short + 4,
            "清单从 3 条增到 30 条，面板从 {short} 行涨到 {long} 行"
        );
    }

    /// 【终端】【底部面板】验证进行中项始终留在展示窗口内。
    #[test]
    fn todo_window_keeps_the_active_item_visible() {
        for focus in [0usize, 7, 19] {
            let rendered = render_panel_lines(&todos(20, focus), &[], &[], 80).join("\n");
            assert!(
                rendered.contains(&format!("› 任务{focus}")),
                "进行中项 {focus} 未出现在窗口内: {rendered}"
            );
        }
    }

    /// 【终端】【底部面板】验证被折叠的条目有数量提示。
    #[test]
    fn hidden_todo_items_are_counted() {
        let rendered = render_panel_lines(&todos(12, 0), &[], &[], 80).join("\n");

        assert!(rendered.contains("0/12"));
        assert!(rendered.contains("+7"));
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
