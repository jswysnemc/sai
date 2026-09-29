/// 【终端】【输入视口】截取光标附近的视觉行，不改变原始文本。
/// 参数: lines 为已折行文本，cursor 为光标行，limit 为可见行上限
/// 返回: 可见行及相对于视口的光标行
pub(super) fn select_rows(lines: &[String], cursor: usize, limit: usize) -> (Vec<String>, u16) {
    let limit = limit.max(1);
    let cursor = cursor.min(lines.len().saturating_sub(1));
    let start = cursor
        .saturating_sub(limit / 2)
        .min(lines.len().saturating_sub(limit));
    (
        lines.iter().skip(start).take(limit).cloned().collect(),
        (cursor - start) as u16,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【输入回归】逐行导航必须始终显示光标位置，包含首尾及单行窗口。
    /// 参数: 无；返回: 无
    #[test]
    fn every_cursor_row_is_visible() {
        let lines = (0..100)
            .map(|index| format!("正文 {index}"))
            .collect::<Vec<_>>();
        for limit in [1, 3, 12, 200] {
            for cursor in 0..lines.len() {
                let (visible, row) = select_rows(&lines, cursor, limit);
                assert_eq!(visible[usize::from(row)], lines[cursor]);
                assert!(visible.len() <= limit);
            }
        }
    }
}
