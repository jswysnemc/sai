/// TUI / CLI 共用的 todo 视觉样式与窗口选取。
///
/// 沉底面板与 transcript 工具卡共用同一套状态符，避免两处漂移。

/// 按状态返回条目标记（含 ANSI）。
///
/// 参数:
/// - `status`: 待办状态
///
/// 返回:
/// - 状态标记
pub(crate) fn status_marker(status: &str) -> &'static str {
    match status {
        "completed" => "\x1b[32m✓\x1b[0m",
        // ▶ 表示「当前焦点」，与用户回显 ●、工具 •、思考 ◦ 区分开
        "in_progress" => "\x1b[1m\x1b[36m▶\x1b[0m",
        "cancelled" => "\x1b[2m✕\x1b[0m",
        _ => "\x1b[2m○\x1b[0m",
    }
}

/// 按状态着色待办正文。
///
/// 已完成与已取消均带删除线，扫读时一眼区分「已关掉」的条目。
///
/// 参数:
/// - `status`: 状态
/// - `text`: 原文
///
/// 返回:
/// - 带 ANSI 的文本
pub(crate) fn colorize_item(status: &str, text: &str) -> String {
    match status {
        "completed" => format!("\x1b[2m\x1b[9m{text}\x1b[0m"),
        "in_progress" => format!("\x1b[1m\x1b[36m{text}\x1b[0m"),
        "cancelled" => format!("\x1b[2m\x1b[9m{text}\x1b[0m"),
        _ => text.to_string(),
    }
}

/// 展示排序：已完成置顶，其次进行中、待办、取消。
///
/// 参数:
/// - `status`: 状态
///
/// 返回:
/// - 排序键（越小越靠前）
pub(crate) fn status_rank(status: &str) -> u8 {
    match status {
        "completed" => 0,
        "in_progress" => 1,
        "pending" | "todo" => 2,
        "cancelled" => 3,
        _ => 4,
    }
}

/// 在「已完成置顶」排序后的清单上选取展示窗口，并保证进行中项可见。
///
/// 参数:
/// - `statuses`: 已按展示顺序排好的状态切片
/// - `limit`: 最多展示条数
///
/// 返回:
/// - 窗口在完整清单中的起止下标（半开区间）
pub(crate) fn display_window(statuses: &[&str], limit: usize) -> (usize, usize) {
    let len = statuses.len();
    if len == 0 || limit == 0 {
        return (0, 0);
    }
    if len <= limit {
        return (0, len);
    }
    let focus = statuses
        .iter()
        .position(|status| *status == "in_progress")
        .unwrap_or_else(|| {
            statuses
                .iter()
                .position(|status| matches!(*status, "pending" | "todo"))
                .unwrap_or(0)
        });
    // 焦点尽量靠窗口下半，让上方多留已完成条目
    let start = focus
        .saturating_sub(limit.saturating_sub(2).max(1))
        .min(len - limit);
    (start, start + limit)
}

/// 按展示顺序重排下标：已完成在前，保持各组内部原序。
///
/// 参数:
/// - `statuses`: 原始状态序列
///
/// 返回:
/// - 重排后的原始下标
pub(crate) fn display_order(statuses: &[&str]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..statuses.len()).collect();
    indices.sort_by_key(|&index| status_rank(statuses[index]));
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_items_sort_above_active_and_pending() {
        let statuses = ["pending", "in_progress", "completed", "completed"];
        let order = display_order(&statuses);
        assert_eq!(
            order
                .iter()
                .map(|&i| statuses[i])
                .collect::<Vec<_>>(),
            vec!["completed", "completed", "in_progress", "pending"]
        );
    }

    #[test]
    fn display_window_keeps_completed_above_active() {
        let statuses = [
            "completed",
            "completed",
            "completed",
            "in_progress",
            "pending",
            "pending",
        ];
        let (start, end) = display_window(&statuses, 4);
        let window = &statuses[start..end];
        assert!(window.contains(&"in_progress"));
        assert_eq!(window[0], "completed");
        assert!(window.iter().position(|s| *s == "completed").unwrap()
            < window.iter().position(|s| *s == "in_progress").unwrap());
    }

    #[test]
    fn completed_text_uses_strikethrough() {
        assert!(colorize_item("completed", "done").contains("\x1b[9m"));
    }

    #[test]
    fn in_progress_marker_is_not_user_or_tool_bullet() {
        let marker = status_marker("in_progress");
        assert!(marker.contains('▶'));
        assert!(!marker.contains('●'));
        assert!(!marker.contains('•'));
    }
}
