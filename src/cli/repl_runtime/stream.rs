use crate::render::transcript::{AnsiLine, DisplayWindow};

/// 已写入终端的 transcript 行快照与滚动进度。
///
/// `offscreen` 之前的全局行已进入原生 scrollback，不再触碰；
/// `[offscreen, total)` 是仍在屏幕上、可以按行修补的受管区域。
#[derive(Default)]
pub(super) struct StreamState {
    total: usize,
    start: usize,
    window: Vec<AnsiLine>,
    offscreen: usize,
}

/// 单次同步的执行计划。
pub(super) enum SyncPlan {
    /// 终端内容与 transcript 一致，无需绘制
    Unchanged,
    /// 增量协调：先修补变化行，再追加新行，最后清理收缩行
    Delta {
        patches: Vec<(usize, AnsiLine)>,
        append: Vec<AnsiLine>,
        old_total: usize,
        new_total: usize,
    },
    /// 窗口未覆盖必要行，退化为可视尾部重绘
    Repaint,
}

impl StreamState {
    /// 返回已滚入原生 scrollback 的行数（也是屏幕上首个受管行的全局行号）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已滚出行数
    pub(super) fn offscreen(&self) -> usize {
        self.offscreen
    }

    /// 返回当前仍在屏幕上的受管行数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 屏幕内受管行数
    pub(super) fn on_screen(&self) -> usize {
        self.total.saturating_sub(self.offscreen)
    }

    /// 记录本次追加造成的终端整体滚动。
    ///
    /// 参数:
    /// - `rows`: 滚动行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn note_scrolled(&mut self, rows: u16) {
        self.offscreen = (self.offscreen + usize::from(rows)).min(self.total);
    }

    /// 将全部受管行标记为已滚出（外部程序写入终端后调用）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn mark_all_offscreen(&mut self) {
        self.offscreen = self.total;
    }

    /// 与最新 transcript 窗口比较，产出增量执行计划。
    ///
    /// 参数:
    /// - `next`: 当前 transcript 尾部窗口
    ///
    /// 返回:
    /// - 增量执行计划
    pub(super) fn sync(&mut self, next: &DisplayWindow) -> SyncPlan {
        let old_total = self.total;
        let new_total = next.total;
        // 1. 快速路径：无脏行且行数不变
        if new_total == old_total && next.dirty_from >= old_total {
            return SyncPlan::Unchanged;
        }
        // 2. 收缩深入 scrollback 区域时无法就地清理，退化为可视尾部重绘
        if new_total < self.offscreen {
            return SyncPlan::Repaint;
        }
        // 3. 修补区间 [dirty_from, min(old,new))：新旧行都可得且内容不同时才重写
        let common = old_total.min(new_total);
        let mut patches = Vec::new();
        for row in next.dirty_from..common {
            let Some(new_line) = next.line_at(row) else {
                continue;
            };
            match self.line_at(row) {
                // 旧行已滚出比较窗口：屏幕上也必然不可达，跳过
                None => continue,
                Some(old_line) if old_line == new_line => continue,
                Some(_) => patches.push((row, new_line.clone())),
            }
        }
        // 4. 追加行必须全部在窗口内，否则窗口覆盖不足，退化为重绘
        let mut append = Vec::with_capacity(new_total.saturating_sub(old_total));
        for row in old_total..new_total {
            let Some(line) = next.line_at(row) else {
                return SyncPlan::Repaint;
            };
            append.push(line.clone());
        }
        self.commit(next);
        if patches.is_empty() && append.is_empty() && new_total == old_total {
            return SyncPlan::Unchanged;
        }
        SyncPlan::Delta {
            patches,
            append,
            old_total,
            new_total,
        }
    }

    /// 用一次完整重放后的窗口状态重置快照。
    ///
    /// 参数:
    /// - `next`: 重放使用的 transcript 窗口
    /// - `painted_rows`: 重放实际绘制在屏幕上的行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn reset(&mut self, next: &DisplayWindow, painted_rows: usize) {
        self.commit(next);
        self.offscreen = next.total.saturating_sub(painted_rows);
    }

    /// 提交窗口数据为当前快照。
    ///
    /// 参数:
    /// - `next`: 最新窗口
    ///
    /// 返回:
    /// - 无
    fn commit(&mut self, next: &DisplayWindow) {
        self.total = next.total;
        self.start = next.start;
        self.window = next.lines.clone();
        self.offscreen = self.offscreen.min(next.total);
    }

    /// 按全局行号读取快照内的行。
    ///
    /// 参数:
    /// - `row`: 全局行号
    ///
    /// 返回:
    /// - 窗口覆盖该行时返回内容
    fn line_at(&self, row: usize) -> Option<&AnsiLine> {
        row.checked_sub(self.start)
            .and_then(|offset| self.window.get(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::{StreamState, SyncPlan};
    use crate::render::transcript::{AnsiLine, DisplayWindow};

    /// 构造覆盖整个行列表的窗口。
    ///
    /// 参数:
    /// - `lines`: 全部行文本
    /// - `dirty_from`: 脏行水位
    ///
    /// 返回:
    /// - 测试窗口
    fn window(lines: &[&str], dirty_from: usize) -> DisplayWindow {
        DisplayWindow {
            total: lines.len(),
            start: 0,
            lines: lines
                .iter()
                .map(|line| AnsiLine::new((*line).to_string()))
                .collect(),
            dirty_from,
        }
    }

    /// 验证纯追加只产出新增行，不修补稳定前缀。
    #[test]
    fn appends_only_new_lines() {
        let mut state = StreamState::default();
        assert!(matches!(
            state.sync(&window(&["first"], 0)),
            SyncPlan::Delta { append, patches, .. } if append.len() == 1 && patches.is_empty()
        ));

        let plan = state.sync(&window(&["first", "second"], 1));
        assert!(matches!(
            plan,
            SyncPlan::Delta { append, patches, .. }
                if append == vec![AnsiLine::new("second".to_string())] && patches.is_empty()
        ));
    }

    /// 验证已渲染行变化时只修补该行，不触发整屏重建。
    #[test]
    fn patches_changed_line_in_place() {
        let mut state = StreamState::default();
        state.sync(&window(&["tool run", "output"], 0));

        let plan = state.sync(&window(&["tool ok", "output"], 0));
        match plan {
            SyncPlan::Delta {
                patches, append, ..
            } => {
                assert_eq!(patches, vec![(0, AnsiLine::new("tool ok".to_string()))]);
                assert!(append.is_empty());
            }
            _ => panic!("expected delta"),
        }
    }

    /// 验证行数收缩时产出新旧总行数，交由执行层清理多余行。
    #[test]
    fn shrink_reports_old_and_new_totals() {
        let mut state = StreamState::default();
        state.sync(&window(&["a", "b", "c"], 0));

        let plan = state.sync(&window(&["a"], 1));
        match plan {
            SyncPlan::Delta {
                old_total,
                new_total,
                ..
            } => {
                assert_eq!(old_total, 3);
                assert_eq!(new_total, 1);
            }
            _ => panic!("expected delta"),
        }
    }

    /// 验证脏水位之后无实际内容变化时返回 Unchanged。
    #[test]
    fn equal_lines_after_dirty_watermark_stay_unchanged() {
        let mut state = StreamState::default();
        state.sync(&window(&["a", "b"], 0));

        assert!(matches!(
            state.sync(&window(&["a", "b"], 0)),
            SyncPlan::Unchanged
        ));
    }

    /// 验证滚动记录使受管行数保持与屏幕一致。
    #[test]
    fn scroll_tracking_reduces_on_screen_rows() {
        let mut state = StreamState::default();
        state.sync(&window(&["a", "b", "c"], 0));
        state.note_scrolled(2);

        assert_eq!(state.offscreen(), 2);
        assert_eq!(state.on_screen(), 1);
    }
}
