//! 右侧用户消息概览轨道：每条消息一个标记，点击跳转，悬停预览。
//!
//! 与 Web 侧栏概览同一思路：标记按消息顺序均匀排布，与文档实际位置无关，
//! 这样长回复不会把后面的消息挤成一团。消息多于轨道行数时相邻消息合并到同一行。

/// 相邻标记的最大间距（行）：消息很少时不把标记撒满整条轨道。
const MAX_SPACING: usize = 2;

/// 一行轨道上的标记。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RailMark {
    /// 相对正文顶部的行
    pub(super) row: usize,
    /// 点击时跳转的用户消息下标
    pub(super) anchor: usize,
    /// 该行覆盖的最后一条用户消息下标（合并时大于 anchor）
    pub(super) last: usize,
}

/// 计算轨道标记位置。
///
/// 参数:
/// - `count`: 用户消息数量
/// - `height`: 轨道可用行数
///
/// 返回:
/// - 自上而下的标记；轨道垂直居中
pub(super) fn rail_marks(count: usize, height: usize) -> Vec<RailMark> {
    if count == 0 || height == 0 {
        return Vec::new();
    }
    if count <= height {
        // 1. 放得下：按最大间距排开后整体居中
        let spacing = if count == 1 {
            0
        } else {
            ((height - 1) / (count - 1)).clamp(1, MAX_SPACING)
        };
        let span = spacing * (count - 1) + 1;
        let top = (height - span) / 2;
        return (0..count)
            .map(|index| RailMark {
                row: top + index * spacing,
                anchor: index,
                last: index,
            })
            .collect();
    }
    // 2. 放不下：每行覆盖一段连续消息
    (0..height)
        .map(|row| {
            let anchor = row * count / height;
            let last = ((row + 1) * count / height).saturating_sub(1).max(anchor);
            RailMark { row, anchor, last }
        })
        .collect()
}

/// 找到覆盖指定用户消息的标记。
///
/// 参数:
/// - `marks`: 轨道标记
/// - `anchor`: 用户消息下标
///
/// 返回:
/// - 标记下标
pub(super) fn mark_for_anchor(marks: &[RailMark], anchor: usize) -> Option<usize> {
    marks
        .iter()
        .position(|mark| anchor >= mark.anchor && anchor <= mark.last)
}

/// 按点击行找到标记；允许点在相邻空行上。
///
/// 参数:
/// - `marks`: 轨道标记
/// - `row`: 相对正文顶部的行
///
/// 返回:
/// - 距离点击行最近且不超过一行的标记
pub(super) fn mark_at_row(marks: &[RailMark], row: usize) -> Option<RailMark> {
    marks
        .iter()
        .filter(|mark| mark.row.abs_diff(row) <= 1)
        .min_by_key(|mark| mark.row.abs_diff(row))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 消息少时标记按两行间距居中排列。
    #[test]
    fn few_messages_are_centered_with_bounded_spacing() {
        let marks = rail_marks(3, 21);
        let rows = marks.iter().map(|mark| mark.row).collect::<Vec<_>>();
        assert_eq!(rows, vec![8, 10, 12]);
        assert_eq!(rail_marks(1, 9)[0].row, 4);
    }

    /// 消息多于轨道行数时每行覆盖一段，且覆盖全部消息不遗漏。
    #[test]
    fn many_messages_share_rows_without_gaps() {
        let marks = rail_marks(50, 10);
        assert_eq!(marks.len(), 10);
        assert_eq!(marks[0].anchor, 0);
        assert_eq!(marks.last().unwrap().last, 49);
        for pair in marks.windows(2) {
            assert_eq!(pair[0].last + 1, pair[1].anchor);
        }
        assert_eq!(mark_for_anchor(&marks, 23), Some(4));
    }

    /// 点击标记旁一行也能命中，远处空行不命中。
    #[test]
    fn clicks_tolerate_one_row() {
        let marks = rail_marks(3, 21);
        assert_eq!(mark_at_row(&marks, 9).map(|mark| mark.anchor), Some(0));
        assert_eq!(mark_at_row(&marks, 12).map(|mark| mark.anchor), Some(2));
        assert_eq!(mark_at_row(&marks, 2), None);
    }
}
