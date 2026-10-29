/// 【终端】【阅读位置】用百分比表达执行链阅读位置，不把视口位置冒充任务完成进度。
/// 参数: `first`、`last` 为可见行，`total` 为总行数；返回: 紧凑位置百分比。
pub(crate) fn render_scroll_position(first: usize, last: usize, total: usize) -> String {
    if total == 0 {
        return "0%".into();
    }
    let visible = last.min(total).saturating_sub(first.saturating_sub(1));
    let scrollable = total.saturating_sub(visible);
    let percent = if scrollable == 0 {
        100
    } else {
        ((first.saturating_sub(1) as u128 * 100) / scrollable as u128).min(100)
    };
    format!("{percent}%")
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 【终端】【阅读位置测试】顶部、中间、底部与空结果均有明确数值。
    /// 参数: 无；返回: 无。
    #[test]
    fn scroll_position_boundaries() {
        assert_eq!(render_scroll_position(1, 10, 40), "0%");
        assert_eq!(render_scroll_position(16, 25, 40), "50%");
        assert_eq!(render_scroll_position(31, 40, 40), "100%");
        assert_eq!(render_scroll_position(1, 40, 40), "100%");
        assert_eq!(render_scroll_position(0, 0, 0), "0%");
    }
}
